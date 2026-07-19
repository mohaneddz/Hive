import { ArrowRight, CalendarDays, MapPin } from "lucide-react";

export function ExhibitionFeature() {
  return (
    <section className="relative overflow-hidden rounded-[26px] bg-[#29271f] p-7 text-white dark:bg-[#0f0f0d]">
      <div className="absolute inset-0 opacity-50" style={{ backgroundImage: "linear-gradient(110deg, transparent 24%, rgba(227,161,5,.4) 24.5%, transparent 25%), radial-gradient(circle at 80% 10%, rgba(248,226,187,.35), transparent 32%)" }} />
      <div className="relative grid grid-cols-[minmax(0,1fr)_250px] items-end gap-6">
        <div className="max-w-xl">
          <p className="eyebrow text-honey">On view now</p>
          <h2 className="mt-3 text-3xl font-extrabold tracking-[-.045em]">The Shape of Quiet</h2>
          <p className="mt-3 max-w-md text-sm leading-relaxed text-white/65">A group exhibition exploring the tension between stillness, colour, and memory.</p>
          <div className="mt-6 flex flex-wrap gap-x-5 gap-y-2 text-xs font-semibold text-white/75">
            <span className="flex items-center gap-2"><CalendarDays size={14} /> 18 Jul — 02 Sep</span>
            <span className="flex items-center gap-2"><MapPin size={14} /> Hive Gallery, Algiers</span>
          </div>
        </div>
        <button className="inline-flex items-center justify-center gap-2 self-end rounded-xl bg-honey px-4 py-3 text-xs font-extrabold text-ink transition hover:bg-honey-dark">Explore exhibition <ArrowRight size={15} /></button>
      </div>
    </section>
  );
}
