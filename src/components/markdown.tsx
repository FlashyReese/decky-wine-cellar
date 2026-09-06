import { FC } from "react";
import { Focusable, Navigation } from "@decky/ui";
import { Options, default as ReactMarkdown } from "react-markdown";
import remarkGfm from "remark-gfm";

interface MarkdownProps extends Options {
  onDismiss?: () => void;
}

export const Markdown: FC<MarkdownProps> = (props) => {
  const { onDismiss, ...markdownProps } = props;

  return (
    <>
      <style>
        {`
          .wine-cellar-markdown-link-focus {
            outline: 2px solid rgba(255, 255, 255, 0.85);
            outline-offset: 3px;
            border-radius: 2px;
          }
        `}
      </style>
      <Focusable>
        <ReactMarkdown
          remarkPlugins={[remarkGfm]}
          components={{
            div: (nodeProps) => (
              <Focusable {...nodeProps.node?.properties}>
                {nodeProps.children}
              </Focusable>
            ),
            a: (nodeProps) => {
              const href = nodeProps.href;
              return (
                <Focusable
                  onActivate={() => {}}
                  onOKButton={() => {
                    if (href == null) {
                      return;
                    }

                    onDismiss?.();
                    Navigation.NavigateToExternalWeb(href);
                  }}
                  style={{ display: "inline" }}
                  focusClassName="wine-cellar-markdown-link-focus"
                >
                  <a href={href} title={nodeProps.title}>
                    {nodeProps.children}
                  </a>
                </Focusable>
              );
            },
          }}
          {...markdownProps}
        >
          {markdownProps.children}
        </ReactMarkdown>
      </Focusable>
    </>
  );
};
