import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";

/** Renders markdown with GFM (tables, strikethrough) using scoped `.whis-md` styles. */
export function Markdown({ children }: { children: string }) {
  return (
    <div className="whis-md">
      <ReactMarkdown remarkPlugins={[remarkGfm]}>{children}</ReactMarkdown>
    </div>
  );
}
