export interface Artwork {
  id: string;
  title: string;
  artist: string;
  year: string;
  medium: string;
  dimensions: string;
  image: string;
  palette: string;
  saved?: boolean;
}

export interface Artist {
  id: string;
  name: string;
  location: string;
  discipline: string;
  portrait: string;
  artworkCount: number;
}
