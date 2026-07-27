import { useEffect, useState } from "react";
import { Plus, Wand2, X } from "lucide-react";

import { Button } from "@/components/ui/Button";
import { createSmartAlbum, listPeople } from "@/lib/tauri";
import type {
  PersonSummary,
  SmartAlbum,
  SmartAlbumMatch,
  SmartAlbumRule,
  SmartAlbumRuleKind,
} from "@/types/media";
import { cn } from "@/utils/cn";

/** Which control the value field turns into for a given rule kind. */
type ValueInput = "text" | "number" | "none" | "dateRange" | "mediaType" | "person";

interface KindSpec {
  kind: SmartAlbumRuleKind;
  label: string;
  input: ValueInput;
  /** Operator keys mirror `rule_to_sql`; `not_` inverts the one that follows. */
  operators: { value: string; label: string }[];
  placeholder?: string;
  hint?: string;
}

const IS_OPERATORS = [
  { value: "is", label: "is" },
  { value: "not_is", label: "is not" },
];

const TEXT_OPERATORS = [
  ...IS_OPERATORS,
  { value: "contains", label: "contains" },
  { value: "not_contains", label: "does not contain" },
];

const CONTAINS_OPERATORS = [
  { value: "contains", label: "contains" },
  { value: "not_contains", label: "does not contain" },
];

const SCORE_OPERATORS = [
  { value: "at_least", label: "is at least" },
  { value: "at_most", label: "is at most" },
  { value: "not_at_least", label: "is not at least" },
];

/**
 * The rule kinds offered by the backend, in the order they are worth reaching
 * for. Kept in sync by hand with `SUPPORTED_RULES` in `smart_albums.rs` — the
 * backend rejects anything it does not know, so a drift shows up as an error
 * rather than as an album that quietly matches too much.
 */
const KINDS: KindSpec[] = [
  { kind: "tag", label: "Tag", input: "text", operators: TEXT_OPERATORS, placeholder: "beach" },
  { kind: "person", label: "Person", input: "person", operators: IS_OPERATORS },
  { kind: "favorite", label: "Favorite", input: "none", operators: IS_OPERATORS },
  { kind: "media_type", label: "Type", input: "mediaType", operators: IS_OPERATORS },
  {
    kind: "date_range",
    label: "Date",
    input: "dateRange",
    operators: [
      { value: "between", label: "between" },
      { value: "not_between", label: "not between" },
    ],
  },
  {
    kind: "place",
    label: "Place",
    input: "text",
    operators: TEXT_OPERATORS,
    placeholder: "Paris",
    hint: "Matches place names already looked up on the Places page.",
  },
  { kind: "camera", label: "Camera", input: "text", operators: TEXT_OPERATORS, placeholder: "X100V" },
  {
    kind: "caption",
    label: "Caption",
    input: "text",
    operators: CONTAINS_OPERATORS,
    placeholder: "dog",
    hint: "Searches the captions the AI wrote.",
  },
  {
    kind: "filename",
    label: "Filename",
    input: "text",
    operators: CONTAINS_OPERATORS,
    placeholder: "IMG_",
  },
  {
    kind: "aesthetic",
    label: "Aesthetic score",
    input: "number",
    operators: SCORE_OPERATORS,
    placeholder: "7",
    hint: "1 to 10. Around 7 is a good photo.",
  },
  {
    kind: "blur",
    label: "Sharpness",
    input: "number",
    operators: SCORE_OPERATORS,
    placeholder: "100",
    hint: "Higher is sharper. Below ~100 usually looks blurry.",
  },
  {
    kind: "nsfw",
    label: "Sensitive score",
    input: "number",
    operators: SCORE_OPERATORS,
    placeholder: "0.7",
    hint: "0 to 1. Above 0.7 is treated as sensitive elsewhere in the app.",
  },
];

const specFor = (kind: string) => KINDS.find((entry) => entry.kind === kind) ?? KINDS[0];

/** A blank rule of the given kind, with a value its input can actually hold. */
function blankRule(kind: SmartAlbumRuleKind): SmartAlbumRule {
  const spec = specFor(kind);
  const value =
    spec.input === "none"
      ? "true"
      : spec.input === "mediaType"
        ? "image"
        : spec.input === "dateRange"
          ? "|"
          : "";
  return { kind, operator: spec.operators[0].value, value };
}

/**
 * Builds a smart album rule by rule. Two rules joined by "all" is an
 * intersection, by "any" a union — the difference between "beach *and* sunset"
 * (often nothing) and "beach *or* sunset".
 */
export function SmartAlbumDialog({
  onClose,
  onCreated,
}: {
  onClose: () => void;
  onCreated: (album: SmartAlbum) => void;
}) {
  const [name, setName] = useState("");
  const [matchType, setMatchType] = useState<SmartAlbumMatch>("all");
  const [rules, setRules] = useState<SmartAlbumRule[]>([blankRule("tag")]);
  const [people, setPeople] = useState<PersonSummary[]>([]);
  const [busy, setBusy] = useState(false);
  const [failure, setFailure] = useState<string | null>(null);

  useEffect(() => {
    void listPeople()
      .then((found) => setPeople(found.filter((person) => person.name)))
      .catch(() => setPeople([]));
  }, []);

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  const patch = (index: number, next: Partial<SmartAlbumRule>) =>
    setRules((prev) => prev.map((rule, i) => (i === index ? { ...rule, ...next } : rule)));

  // Changing the kind resets the operator and value: "contains" means nothing to
  // a score, and a date range left in a tag field would be rejected on save.
  const changeKind = (index: number, kind: SmartAlbumRuleKind) =>
    setRules((prev) => prev.map((rule, i) => (i === index ? blankRule(kind) : rule)));

  const submit = async () => {
    const albumName = name.trim();
    if (!albumName) return;
    setBusy(true);
    setFailure(null);
    try {
      onCreated(await createSmartAlbum(albumName, rules, matchType));
      onClose();
    } catch (cause) {
      // The backend refuses rules it cannot translate, so its message is the
      // useful one: it names the rule and lists what would have worked.
      setFailure(String(cause));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div
      className="fixed inset-0 z-50 grid place-items-center bg-black/45 p-6 backdrop-blur-sm"
      onClick={onClose}
    >
      <div
        className="max-h-[88vh] w-full max-w-2xl overflow-y-auto rounded-[22px] border border-ink/[.08] bg-panel p-6 shadow-2xl"
        onClick={(event) => event.stopPropagation()}
      >
        <div className="flex items-start justify-between gap-4">
          <div>
            <p className="eyebrow">Collections</p>
            <h2 className="mt-1.5 text-lg font-extrabold text-ink">New smart album</h2>
            <p className="mt-0.5 text-xs text-ink-muted">
              Saves the rules, not the photos — it keeps itself up to date.
            </p>
          </div>
          <button onClick={onClose} className="icon-button" aria-label="Close">
            <X size={15} />
          </button>
        </div>

        <input
          autoFocus
          value={name}
          onChange={(event) => setName(event.target.value)}
          placeholder="Album name"
          className="search-input mt-5 !pl-3.5"
        />

        <div className="mt-4 flex flex-wrap items-center gap-2 text-xs text-ink-muted">
          <span>Include a photo when it matches</span>
          {(["all", "any"] as const).map((option) => (
            <button
              key={option}
              onClick={() => setMatchType(option)}
              className={cn(
                "rounded-xl border px-3 py-1.5 font-bold transition",
                matchType === option
                  ? "border-honey/50 bg-honey/15 text-honey-deep"
                  : "border-ink/[.08] bg-canvas text-ink-soft hover:border-honey/40",
              )}
            >
              {option === "all" ? "all rules" : "any rule"}
            </button>
          ))}
        </div>

        <div className="mt-4 space-y-2">
          {rules.map((rule, index) => {
            const spec = specFor(rule.kind);
            const [start, end] = rule.value.split("|");
            return (
              <div
                key={index}
                className="rounded-2xl border border-ink/[.08] bg-canvas p-3"
              >
                <div className="flex flex-wrap items-center gap-2">
                  <select
                    value={rule.kind}
                    onChange={(event) =>
                      changeKind(index, event.target.value as SmartAlbumRuleKind)
                    }
                    className="select-input"
                  >
                    {KINDS.map((entry) => (
                      <option key={entry.kind} value={entry.kind}>
                        {entry.label}
                      </option>
                    ))}
                  </select>

                  <select
                    value={rule.operator}
                    onChange={(event) => patch(index, { operator: event.target.value })}
                    className="select-input"
                  >
                    {spec.operators.map((operator) => (
                      <option key={operator.value} value={operator.value}>
                        {operator.label}
                      </option>
                    ))}
                  </select>

                  {spec.input === "none" && (
                    <span className="text-xs text-ink-muted">— no value needed</span>
                  )}

                  {spec.input === "mediaType" && (
                    <select
                      value={rule.value}
                      onChange={(event) => patch(index, { value: event.target.value })}
                      className="select-input"
                    >
                      <option value="image">photo</option>
                      <option value="video">video</option>
                    </select>
                  )}

                  {spec.input === "person" && (
                    <select
                      value={rule.value}
                      onChange={(event) => patch(index, { value: event.target.value })}
                      className="select-input"
                    >
                      <option value="">Pick someone…</option>
                      {people.map((person) => (
                        <option key={person.id} value={person.id}>
                          {person.name}
                        </option>
                      ))}
                    </select>
                  )}

                  {spec.input === "dateRange" && (
                    <>
                      <input
                        type="date"
                        value={start ?? ""}
                        onChange={(event) =>
                          patch(index, { value: `${event.target.value}|${end ?? ""}` })
                        }
                        className="select-input"
                      />
                      <span className="text-xs text-ink-muted">and</span>
                      <input
                        type="date"
                        value={end ?? ""}
                        onChange={(event) =>
                          patch(index, { value: `${start ?? ""}|${event.target.value}` })
                        }
                        className="select-input"
                      />
                    </>
                  )}

                  {(spec.input === "text" || spec.input === "number") && (
                    <input
                      type={spec.input === "number" ? "number" : "text"}
                      step="any"
                      value={rule.value}
                      onChange={(event) => patch(index, { value: event.target.value })}
                      placeholder={spec.placeholder}
                      className="select-input min-w-40 flex-1"
                    />
                  )}

                  {rules.length > 1 && (
                    <button
                      onClick={() => setRules((prev) => prev.filter((_, i) => i !== index))}
                      className="icon-button !h-7 !w-7 shrink-0"
                      aria-label="Remove rule"
                    >
                      <X size={12} />
                    </button>
                  )}
                </div>
                {spec.hint && <p className="mt-2 text-[11px] text-ink-muted">{spec.hint}</p>}
              </div>
            );
          })}
        </div>

        <button
          onClick={() => setRules((prev) => [...prev, blankRule("tag")])}
          className="mt-3 inline-flex items-center gap-2 rounded-xl border border-ink/[.08] bg-canvas px-3 py-2 text-xs font-bold text-ink-soft transition hover:border-honey/40"
        >
          <Plus size={13} /> Add rule
        </button>

        {failure && (
          <p className="mt-4 rounded-xl border border-red-500/30 bg-red-500/10 p-3 text-xs text-red-600">
            {failure}
          </p>
        )}

        <div className="mt-5 flex justify-end gap-2">
          <Button variant="secondary" onClick={onClose}>
            Cancel
          </Button>
          <Button
            onClick={submit}
            disabled={busy || !name.trim()}
            icon={<Wand2 size={15} />}
          >
            Create
          </Button>
        </div>
      </div>
    </div>
  );
}
