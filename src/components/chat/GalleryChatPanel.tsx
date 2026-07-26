import { useEffect, useRef, useState } from "react";
import { Link } from "react-router-dom";
import { Loader2, Send, Sparkles } from "lucide-react";

import { Card } from "@/components/ui/Card";
import { MediaThumb } from "@/components/media/MediaThumb";
import { useAiStatus } from "@/hooks/useAiStatus";
import { galleryChat } from "@/lib/tauri";
import { cn } from "@/utils/cn";

interface ChatMessage {
  role: "user" | "assistant";
  text: string;
  mediaIds?: string[];
}

export function GalleryChatPanel() {
  const { status: aiStatus } = useAiStatus();
  const [messages, setMessages] = useState<ChatMessage[]>([]);
  const [input, setInput] = useState("");
  const [sending, setSending] = useState(false);
  const scrollRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    scrollRef.current?.scrollTo({ top: scrollRef.current.scrollHeight, behavior: "smooth" });
  }, [messages]);

  const send = async () => {
    const text = input.trim();
    if (!text || sending) return;
    setInput("");
    setMessages((prev) => [...prev, { role: "user", text }]);
    setSending(true);
    try {
      const response = await galleryChat(text);
      setMessages((prev) => [...prev, { role: "assistant", text: response.answer, mediaIds: response.mediaIds }]);
    } catch (error) {
      setMessages((prev) => [
        ...prev,
        { role: "assistant", text: error instanceof Error ? error.message : "Something went wrong." },
      ]);
    } finally {
      setSending(false);
    }
  };

  if (!aiStatus?.llmModelsReady) {
    return (
      <Card className="flex flex-col items-center gap-3 p-8 text-center">
        <div className="grid size-11 place-items-center rounded-2xl bg-cream text-honey-deep">
          <Sparkles size={19} />
        </div>
        <p className="text-sm font-extrabold text-ink">Gallery chat isn't set up yet</p>
        <p className="text-xs text-ink-muted">
          Ask questions about your library in plain language — answered by a local model, on this
          device. Download it from Settings to turn this on.
        </p>
        <Link to="/settings" className="text-xs font-bold text-honey-deep hover:underline">
          Go to Settings
        </Link>
      </Card>
    );
  }

  return (
    <Card className="flex h-full flex-col p-4">
      <div className="flex items-center gap-2 border-b border-ink/[.07] pb-3">
        <Sparkles size={15} className="text-honey-deep" />
        <h2 className="text-sm font-extrabold text-ink">Gallery chat</h2>
      </div>

      <div ref={scrollRef} className="mt-3 flex-1 space-y-3 overflow-y-auto">
        {messages.length === 0 && (
          <p className="text-xs text-ink-muted">
            Try asking "What did I photograph last weekend?" or "Do I have any photos with
            handwritten notes?"
          </p>
        )}
        {messages.map((message, i) => (
          <div key={i} className={cn("flex", message.role === "user" ? "justify-end" : "justify-start")}>
            <div
              className={cn(
                "max-w-[85%] rounded-2xl px-3 py-2 text-xs",
                message.role === "user" ? "bg-honey/15 text-ink" : "bg-canvas text-ink",
              )}
            >
              <p className="whitespace-pre-wrap">{message.text}</p>
              {message.mediaIds && message.mediaIds.length > 0 && (
                <div className="mt-2 grid grid-cols-4 gap-1.5">
                  {message.mediaIds.slice(0, 8).map((id) => (
                    <Link key={id} to={`/media/${id}`} className="artwork-frame block aspect-square">
                      <MediaThumb mediaId={id} alt="Referenced photo" className="size-full object-cover" />
                    </Link>
                  ))}
                </div>
              )}
            </div>
          </div>
        ))}
        {sending && (
          <div className="flex items-center gap-2 text-xs text-ink-muted">
            <Loader2 size={13} className="animate-spin" /> Thinking…
          </div>
        )}
      </div>

      <div className="mt-3 flex items-center gap-2 border-t border-ink/[.07] pt-3">
        <input
          value={input}
          onChange={(event) => setInput(event.target.value)}
          onKeyDown={(event) => event.key === "Enter" && send()}
          placeholder="Ask about your photos…"
          disabled={sending}
          className="h-9 flex-1 rounded-xl border border-ink/[.12] bg-panel px-3 text-xs text-ink outline-none focus:border-honey/50"
        />
        <button
          onClick={send}
          disabled={sending || !input.trim()}
          className="grid size-9 shrink-0 place-items-center rounded-xl bg-honey text-[#3b2900] transition hover:bg-honey-dark disabled:pointer-events-none disabled:opacity-50"
          aria-label="Send"
        >
          <Send size={14} />
        </button>
      </div>
    </Card>
  );
}
