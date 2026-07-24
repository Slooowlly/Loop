import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useTranslation } from "react-i18next";

import { buildInboxMessages } from "../../pages/tabs/inboxMessages";

// Caixa de e-mail da revista. Caixa de entrada real: fatos do save
// (confronto direto + favorito) → texto PT.
function MagazineMailbox({ careerId }) {
  const { t } = useTranslation();
  const [messages, setMessages] = useState([]);
  const [selectedId, setSelectedId] = useState(null);
  const [readIds, setReadIds] = useState(() => new Set());

  useEffect(() => {
    let mounted = true;
    if (!careerId) {
      setMessages([]);
      return undefined;
    }
    invoke("get_inbox_messages", { careerId })
      .then((facts) => {
        if (!mounted) return;
        const list = buildInboxMessages(facts);
        setMessages(list);
        setSelectedId((cur) => cur ?? list[0]?.id ?? null);
      })
      .catch(() => {
        if (mounted) setMessages([]);
      });
    return () => {
      mounted = false;
    };
  }, [careerId]);

  const unread = messages.filter((m) => !readIds.has(m.id)).length;
  const selected = messages.find((m) => m.id === selectedId) ?? null;

  function selectMessage(id) {
    setSelectedId(id);
    setReadIds((prev) => {
      const nextSet = new Set(prev);
      nextSet.add(id);
      return nextSet;
    });
  }

  return (
    <section className="mailbox">
      <div className="mb-head">
        <span className="mb-icon">✉</span>
        <span className="mb-title">{t("newsMagazine.mailbox.inbox")}</span>
        {unread > 0 && <span className="mb-count">{unread}</span>}
      </div>

      <div className="mb-split">
        <div className="mb-list">
          {messages.map((m) => {
            const classes = ["mrow"];
            if (readIds.has(m.id)) classes.push("read");
            if (m.id === selectedId) classes.push("active");
            return (
              <div
                key={m.id}
                className={classes.join(" ")}
                onClick={() => selectMessage(m.id)}
                role="button"
                tabIndex={0}
              >
                <span className={`mava ${m.av}`}>{m.ini}</span>
                <div className="m-main">
                  <span className="mfrom">
                    {m.from}
                    <small>{m.kind}</small>
                  </span>
                </div>
                <span className="mright">
                  <span className="mtime">{m.time}</span>
                  <span className="ndot" />
                </span>
              </div>
            );
          })}
        </div>

        {selected ? (
          <div className="mb-reader">
            <div className="reader-head">
              <span className={`mava ${selected.av}`}>{selected.ini}</span>
              <span className="reader-from">
                {selected.from}
                <small>{selected.kind}</small>
              </span>
              <span className="reader-time">{selected.time}</span>
            </div>
            <h3 className="reader-subject">{selected.subject}</h3>
            <div className="reader-body" dangerouslySetInnerHTML={{ __html: selected.body }} />
            {selected.actions.length > 0 && (
              <div className="reader-actions">
                {selected.actions.map((a) => (
                  <button key={a.label} type="button" className={`mbtn ${a.type}`}>
                    {a.label}
                  </button>
                ))}
              </div>
            )}
          </div>
        ) : (
          <div className="mb-reader empty">
            <div>
              <div className="ph-ic">✉</div>
              {t("newsMagazine.mailbox.selectPrompt")}
            </div>
          </div>
        )}
      </div>
    </section>
  );
}

export default MagazineMailbox;
