import { useTranslation } from "react-i18next";

import { PlayerSkillSection } from "../../../components/driver/detalhes/PlayerSkillSection.jsx";
import { technicalToneClass } from "../../../components/driver/detalhes/primitivos.jsx";
import { formatAverage } from "../../../components/driver/detalhes/formatadores.js";
import { Bloco, Numero, Vazio } from "./primitivos.jsx";

// F-02 — a ficha do MEU piloto.
//
// Tudo aqui já existia no jogo e nada disso tinha lugar próprio: os atributos
// inferidos moravam numa aba dentro do modal que serve para olhar qualquer piloto
// do grid, a fase do arco só aparecia de esguelha, e motivação, licença e lesão
// estavam espalhadas por três telas que aparecem uma vez e somem.
//
// O que a seção NÃO repete: números de carreira (corridas, vitórias, pódios) e
// títulos moram em Troféus, e a trajetória em História. Esta responde só "como eu
// estou e do que eu sou feito hoje" — e por isso abre com a habilidade medida, que
// é a única leitura do jogo que existe para o jogador e não existe para a IA.
function MeuPilotoSection({ detail, careerId }) {
  const { t } = useTranslation();
  const perfil = detail.perfil ?? {};
  const competitivo = detail.competitivo ?? {};
  const arco = detail.arco ?? {};
  const forma = detail.forma ?? {};
  const resumo = detail.resumo_atual ?? {};
  const estrelato = detail.estrelato ?? {};
  const lesao = detail.saude?.lesao_ativa ?? null;
  const personalidades = [
    competitivo.personalidade_primaria,
    competitivo.personalidade_secundaria,
  ].filter(Boolean);
  const leituraTecnica = detail.leitura_tecnica?.itens ?? [];

  return (
    <div className="space-y-4">
      {/* A habilidade medida vem PRIMEIRO. Ela é a resposta à pergunta que só o
          protagonista faz — "quanto eu valho de fato?" — e é derivada das corridas
          que o jogador realmente correu, não de um atributo escrito no save. */}
      <PlayerSkillSection SectionComponent={BlocoComTitulo} careerId={careerId} />

      <div className="grid gap-4 lg:grid-cols-2">
        <Bloco titulo={t("carreiraTab.piloto.now")} testId="carreira-piloto-agora">
          <div className="grid grid-cols-2 gap-4 sm:grid-cols-4">
            <Numero
              valor={resumo.posicao_campeonato ? `P${resumo.posicao_campeonato}` : "-"}
              rotulo={t("carreiraTab.header.championship")}
            />
            <Numero valor={resumo.pontos ?? 0} rotulo={t("carreiraTab.piloto.points")} />
            <Numero
              valor={formatAverage(resumo.media_recente)}
              rotulo={t("carreiraTab.piloto.recentAverage")}
            />
            <Numero
              valor={perfil.licenca?.sigla || "-"}
              rotulo={t("carreiraTab.piloto.license")}
              nota={perfil.licenca?.nivel || null}
            />
          </div>

          {resumo.veredito ? (
            <p className="mt-4 border-t border-white/[0.08] pt-3 text-sm leading-relaxed text-text-secondary">
              <strong className="font-semibold text-text-primary">{resumo.veredito}</strong>
              {forma.contexto ? ` · ${forma.contexto}` : ""}
            </p>
          ) : null}

          {lesao ? (
            <div
              data-testid="carreira-piloto-lesao"
              className="mt-3 rounded-xl border border-status-red/30 bg-status-red/10 px-3.5 py-2.5"
            >
              <strong className="block text-sm font-semibold text-status-red">
                {lesao.nome || t("carreiraTab.piloto.injuryFallback")}
              </strong>
              <p className="mt-1 text-xs leading-relaxed text-text-secondary">
                {t("carreiraTab.piloto.injuryDetail", {
                  remaining: lesao.corridas_restantes,
                  total: lesao.corridas_total,
                })}
                {lesao.corrida_ocorrida_pista
                  ? ` · ${t("carreiraTab.piloto.injuryWhere", {
                      track: lesao.corrida_ocorrida_pista,
                    })}`
                  : ""}
              </p>
            </div>
          ) : null}
        </Bloco>

        <Bloco titulo={t("carreiraTab.piloto.arc")} testId="carreira-piloto-arco">
          {arco.fase ? (
            <>
              <div className="flex flex-wrap items-baseline gap-x-3 gap-y-1">
                <strong
                  className={`text-lg font-semibold ${
                    technicalToneClass[arco.tom_fase] ?? technicalToneClass.neutral
                  }`}
                >
                  {arco.fase}
                </strong>
                <span className="font-mono text-sm text-text-secondary">
                  {t("driverDetail.profile.age", { count: arco.idade ?? perfil.idade })}
                </span>
              </div>
              {arco.resumo ? (
                <p className="mt-2 text-sm leading-relaxed text-text-secondary">{arco.resumo}</p>
              ) : null}
              <dl className="mt-3.5 grid gap-x-6 gap-y-2 border-t border-white/[0.08] pt-3 sm:grid-cols-2">
                <Linha
                  rotulo={t("carreiraTab.piloto.experience")}
                  valor={arco.nivel_experiencia}
                />
                <Linha
                  rotulo={t("carreiraTab.piloto.development")}
                  valor={arco.nivel_desenvolvimento}
                />
                {/* `nivel_margem` é `None` quando o teto pessoal nunca foi derivado —
                    e o do jogador nunca é. Sem esta guarda a linha diria "chegou ao
                    teto" para quem não tem teto medido. */}
                {arco.nivel_margem ? (
                  <Linha rotulo={t("carreiraTab.piloto.headroom")} valor={arco.nivel_margem} />
                ) : null}
              </dl>
            </>
          ) : (
            <Vazio>{t("carreiraTab.piloto.arcEmpty")}</Vazio>
          )}
        </Bloco>
      </div>

      <div className="grid gap-4 lg:grid-cols-2">
        <Bloco titulo={t("carreiraTab.piloto.personality")}>
          {personalidades.length ? (
            <div className="grid gap-3">
              {personalidades.map((personalidade, indice) => (
                <div
                  key={`${personalidade.tipo}-${indice}`}
                  className="flex items-center gap-4 rounded-xl bg-[#0f1c2b] px-3.5 py-3"
                >
                  <span className="shrink-0 text-[30px] leading-none">{personalidade.emoji}</span>
                  <div className="min-w-0">
                    <strong className="block truncate text-sm font-semibold text-text-primary">
                      {personalidade.tipo}
                    </strong>
                    <p className="mt-1 text-xs leading-5 text-text-secondary">
                      {personalidade.descricao}
                    </p>
                  </div>
                </div>
              ))}
            </div>
          ) : (
            <Vazio>{t("driverDetail.personality.empty")}</Vazio>
          )}
        </Bloco>

        <Bloco titulo={t("carreiraTab.piloto.stardom")}>
          <div className="grid grid-cols-2 gap-4">
            <Numero
              valor={estrelato.fama ?? 0}
              rotulo={t("driverDetail.stardom.fame")}
              nota={estrelato.nivel_fama || null}
            />
            <Numero
              valor={estrelato.carisma ?? 0}
              rotulo={t("carreiraTab.piloto.charisma")}
              nota={estrelato.nivel_carisma || null}
            />
          </div>
          {estrelato.resumo ? (
            <p className="mt-3.5 border-t border-white/[0.08] pt-3 text-sm leading-relaxed text-text-secondary">
              {estrelato.resumo}
            </p>
          ) : null}
        </Bloco>
      </div>

      {leituraTecnica.length ? (
        <Bloco titulo={t("carreiraTab.piloto.technicalRead")} testId="carreira-piloto-tecnica">
          {/* A leitura técnica sai dos ATRIBUTOS, não das corridas do ano: ela diz a
              mesma coisa em janeiro e em dezembro. Fica no fim da seção por isso —
              é retrato, não estado. A referência do grid vem do backend quando
              existe; sem ela a linha mostra só o nível, porque "Instável" sozinho
              não tem contra quem ser instável. */}
          <div className="grid gap-2 sm:grid-cols-2">
            {leituraTecnica.map((item) => (
              <div
                key={item.chave}
                className="flex items-center justify-between gap-3 rounded-lg border border-white/[0.06] bg-black/10 px-3 py-2"
              >
                <span className="min-w-0 truncate text-xs text-text-secondary">{item.label}</span>
                <span className="flex shrink-0 items-baseline gap-2">
                  <span
                    className={`text-xs font-semibold ${
                      technicalToneClass[item.tom] ?? technicalToneClass.neutral
                    }`}
                  >
                    {item.nivel}
                  </span>
                  {Number.isFinite(item.referencia) ? (
                    <span className="font-mono text-[10px] text-text-muted">
                      {t("carreiraTab.piloto.gridReference", { value: item.referencia })}
                    </span>
                  ) : null}
                </span>
              </div>
            ))}
          </div>
        </Bloco>
      ) : null}
    </div>
  );
}

// Adaptador para o `PlayerSkillSection`, que é do modal e pede um componente de
// seção com a prop `title`. Traduzir o nome aqui é mais barato que mudar a
// assinatura de um componente que já tem outro consumidor em produção.
function BlocoComTitulo({ title, children }) {
  return (
    <Bloco titulo={title} testId="carreira-piloto-habilidade">
      {children}
    </Bloco>
  );
}

function Linha({ rotulo, valor }) {
  return (
    <div className="flex items-baseline justify-between gap-3">
      <dt className="text-[11px] uppercase tracking-[0.14em] text-text-muted">{rotulo}</dt>
      <dd className="text-right text-sm font-medium text-text-primary">{valor || "-"}</dd>
    </div>
  );
}

export default MeuPilotoSection;
