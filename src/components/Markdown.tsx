import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";

// Renders Markdown safely: raw HTML in the text is NOT executed (react-markdown
// default), and links don't navigate the app window away.
export default function Markdown({ text }: { text: string }) {
  return (
    <div className="markdown">
      <ReactMarkdown
        remarkPlugins={[remarkGfm]}
        components={{
          a: ({ href, children }) => (
            <a
              href={href}
              title={href}
              onClick={(e) => {
                e.preventDefault();
                if (href) navigator.clipboard?.writeText(href).catch(() => {});
              }}
            >
              {children}
            </a>
          ),
        }}
      >
        {text}
      </ReactMarkdown>
    </div>
  );
}
