import { FC, useRef } from "react";
import { Focusable, Navigation } from "@decky/ui";
import { Options, default as ReactMarkdown } from "react-markdown";
import remarkGfm from "remark-gfm";

interface MarkdownProps extends Options {
  onDismiss?: () => void;
}

export const Markdown: FC<MarkdownProps> = (props) => {
  return (
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
            const aRef = useRef<HTMLAnchorElement>(null);
            return (
              // TODO fix focus ring
              <Focusable
                onActivate={() => {}}
                onOKButton={() => {
                  props.onDismiss?.();
                  Navigation.NavigateToExternalWeb(aRef.current!.href);
                }}
                style={{ display: "inline" }}
              >
                <a ref={aRef} {...nodeProps.node?.properties}>
                  {nodeProps.children}
                </a>
              </Focusable>
            );
          },
        }}
        {...props}
      >
        {props.children}
      </ReactMarkdown>
    </Focusable>
  );
};
