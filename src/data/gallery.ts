import type { Artist, Artwork } from "@/types/gallery";

export const artworks: Artwork[] = [
  { id: "night-garden", title: "Night Garden", artist: "Mira Valen", year: "2024", medium: "Oil on linen", dimensions: "120 × 90 cm", image: "https://images.unsplash.com/photo-1579783902614-a3fb3927b6a5?auto=format&fit=crop&w=1200&q=85", palette: "from-[#233542] via-[#5a786c] to-[#d5b46a]", saved: true },
  { id: "stillness", title: "Stillness in Blue", artist: "Antoine Morel", year: "2023", medium: "Acrylic and pigment", dimensions: "100 × 100 cm", image: "https://images.unsplash.com/photo-1561214115-f2f134cc4912?auto=format&fit=crop&w=1200&q=85", palette: "from-[#435b72] via-[#8ba7bb] to-[#e7d8bc]" },
  { id: "woven-light", title: "Woven Light", artist: "Sofia Okafor", year: "2024", medium: "Textile, dyed cotton", dimensions: "80 × 115 cm", image: "https://images.unsplash.com/photo-1578301978018-3005759f48f7?auto=format&fit=crop&w=1200&q=85", palette: "from-[#b96032] via-[#e9a649] to-[#f6df9e]", saved: true },
  { id: "undertow", title: "Undertow", artist: "Kian Bell", year: "2022", medium: "Archival inkjet print", dimensions: "70 × 100 cm", image: "https://images.unsplash.com/photo-1577083288073-40892c0860a4?auto=format&fit=crop&w=1200&q=85", palette: "from-[#111c2d] via-[#36546d] to-[#9cb3bd]" },
  { id: "yellow-room", title: "The Yellow Room", artist: "Mira Valen", year: "2023", medium: "Oil on canvas", dimensions: "110 × 85 cm", image: "https://images.unsplash.com/photo-1561214115-f2f134cc4912?auto=format&fit=crop&w=1200&q=85", palette: "from-[#9c671f] via-[#dcae4a] to-[#f4e0ae]" },
  { id: "after-rain", title: "After Rain", artist: "Sofia Okafor", year: "2024", medium: "Gouache on paper", dimensions: "56 × 76 cm", image: "https://images.unsplash.com/photo-1549490349-8643362247b5?auto=format&fit=crop&w=1200&q=85", palette: "from-[#273c45] via-[#779299] to-[#cfd4bb]", saved: true },
];

export const artists: Artist[] = [
  { id: "mira", name: "Mira Valen", location: "Copenhagen, Denmark", discipline: "Painting", portrait: "https://images.unsplash.com/photo-1544005313-94ddf0286df2?auto=format&fit=crop&w=400&q=85", artworkCount: 18 },
  { id: "antoine", name: "Antoine Morel", location: "Paris, France", discipline: "Mixed media", portrait: "https://images.unsplash.com/photo-1500648767791-00dcc994a43e?auto=format&fit=crop&w=400&q=85", artworkCount: 12 },
  { id: "sofia", name: "Sofia Okafor", location: "Lagos, Nigeria", discipline: "Textile", portrait: "https://images.unsplash.com/photo-1534528741775-53994a69daeb?auto=format&fit=crop&w=400&q=85", artworkCount: 24 },
];

export async function getArtworks() { return structuredClone(artworks); }
export async function getArtists() { return structuredClone(artists); }
