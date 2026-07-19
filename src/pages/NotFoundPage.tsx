import { Link } from "react-router-dom";

export function NotFoundPage() {
  return (
    <div className="grid min-h-[65vh] place-items-center text-center">
      <div>
        <p className="text-7xl font-black text-cream">404</p>
        <h1 className="mt-2 text-2xl font-extrabold text-ink">This cell is empty.</h1>
        <p className="mt-2 text-sm text-ink-muted">The page you requested does not exist.</p>
        <Link
          to="/"
          className="mt-5 inline-flex h-10 items-center justify-center rounded-xl bg-honey px-4 text-sm font-bold text-ink shadow-[0_8px_20px_rgba(227,161,5,.2)] transition hover:-translate-y-px hover:bg-honey-dark"
        >
          Back to overview
        </Link>
      </div>
    </div>
  );
}
