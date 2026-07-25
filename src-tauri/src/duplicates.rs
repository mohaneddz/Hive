use image_hasher::ImageHash;
use rusqlite::{params, Connection};
use std::collections::HashMap;

/// Hamming-distance threshold (out of 64 bits) below which two images are considered duplicates.
/// image_hasher's default 8x8 gradient hash tolerates minor recompression/resizing differences
/// up to roughly this distance while still being distinct from unrelated images.
const DUPLICATE_THRESHOLD: u32 = 8;

struct UnionFind {
    parent: Vec<usize>,
}

impl UnionFind {
    fn new(n: usize) -> Self {
        Self { parent: (0..n).collect() }
    }

    fn find(&mut self, x: usize) -> usize {
        if self.parent[x] != x {
            self.parent[x] = self.find(self.parent[x]);
        }
        self.parent[x]
    }

    fn union(&mut self, a: usize, b: usize) {
        let (ra, rb) = (self.find(a), self.find(b));
        if ra != rb {
            self.parent[ra] = rb;
        }
    }
}

/// Recomputes duplicate groups from perceptual hashes stored on `media_items` during indexing.
/// This is a full O(n^2) recompute over the eligible image set — fine for personal-library
/// scale (thousands of photos); a large-scale library would want an LSH/bucketing index instead.
pub fn recompute_duplicate_groups(conn: &Connection) -> anyhow::Result<usize> {
    let mut stmt = conn.prepare(
        "SELECT id, phash FROM media_items
         WHERE media_type = 'image' AND is_trashed = 0 AND phash IS NOT NULL",
    )?;
    let rows: Vec<(String, String)> = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    drop(stmt);

    let hashes: Vec<(String, ImageHash)> = rows
        .into_iter()
        .filter_map(|(id, encoded)| ImageHash::from_base64(&encoded).ok().map(|h| (id, h)))
        .collect();

    let n = hashes.len();
    let mut uf = UnionFind::new(n);
    for i in 0..n {
        for j in (i + 1)..n {
            if hashes[i].1.dist(&hashes[j].1) <= DUPLICATE_THRESHOLD {
                uf.union(i, j);
            }
        }
    }

    let mut clusters: HashMap<usize, Vec<usize>> = HashMap::new();
    for i in 0..n {
        clusters.entry(uf.find(i)).or_default().push(i);
    }

    conn.execute("DELETE FROM duplicates", [])?;

    let mut group_count = 0;
    for members in clusters.values() {
        if members.len() < 2 {
            continue;
        }
        group_count += 1;
        let group_id = uuid::Uuid::new_v4().to_string();
        let reference = &hashes[members[0]].1;
        for &idx in members {
            let (media_id, hash) = &hashes[idx];
            let dist = reference.dist(hash);
            let similarity = 1.0 - (dist as f64 / 64.0);
            conn.execute(
                "INSERT INTO duplicates (id, group_id, media_id, similarity) VALUES (?1, ?2, ?3, ?4)",
                params![uuid::Uuid::new_v4().to_string(), group_id, media_id, similarity],
            )?;
        }
    }

    Ok(group_count)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::indexing;

    fn make_gradient_image(path: &std::path::Path, seed: u8) {
        let img = image::RgbImage::from_fn(64, 64, |x, y| {
            image::Rgb([
                ((x * 4) as u8).wrapping_add(seed),
                ((y * 4) as u8).wrapping_add(seed),
                128,
            ])
        });
        img.save(path).unwrap();
    }

    fn make_noise_image(path: &std::path::Path, seed: u32) {
        let mut state = seed;
        let img = image::RgbImage::from_fn(64, 64, |_, _| {
            state = state.wrapping_mul(1_103_515_245).wrapping_add(12_345);
            let v = (state >> 16) as u8;
            image::Rgb([v, v.wrapping_add(64), v.wrapping_add(128)])
        });
        img.save(path).unwrap();
    }

    #[test]
    fn clusters_near_identical_images_and_separates_distinct_ones() {
        let dir = std::env::temp_dir().join(format!("hive_dup_test_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();

        let original = dir.join("a_original.png");
        let near_dup = dir.join("b_near_dup.png");
        let unrelated = dir.join("c_unrelated.png");
        make_gradient_image(&original, 0);
        make_gradient_image(&near_dup, 1); // tiny per-pixel offset, should still hash as near-identical
        make_noise_image(&unrelated, 42);

        let db_path = dir.join("test.db");
        let conn = crate::db::open(&db_path).unwrap();
        conn.execute(
            "INSERT INTO folders (id, path, name, is_watched, added_at) VALUES ('f1', ?1, 'test', 1, '2024-01-01')",
            params![dir.to_string_lossy()],
        )
        .unwrap();

        for path in [&original, &near_dup, &unrelated] {
            indexing::index_file(&conn, "f1", path).unwrap();
        }

        let group_count = recompute_duplicate_groups(&conn).unwrap();
        assert_eq!(group_count, 1, "expected exactly one duplicate cluster");

        let mut stmt = conn.prepare("SELECT m.filename FROM duplicates d JOIN media_items m ON m.id = d.media_id").unwrap();
        let members: Vec<String> = stmt
            .query_map([], |r| r.get(0))
            .unwrap()
            .collect::<rusqlite::Result<Vec<_>>>()
            .unwrap();
        assert_eq!(members.len(), 2, "cluster should contain the two near-identical images");
        assert!(members.iter().any(|f| f.contains("a_original")));
        assert!(members.iter().any(|f| f.contains("b_near_dup")));
        assert!(!members.iter().any(|f| f.contains("c_unrelated")));

        let _ = std::fs::remove_dir_all(&dir);
    }
}
