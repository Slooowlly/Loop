import {
  CloudRain,
  Flame,
  Gauge,
  GraduationCap,
  HeartPulse,
  Magnet,
  Route,
  SlidersHorizontal,
  TrendingUp,
} from "lucide-react";
import { useTranslation } from "react-i18next";

// O CORPO DO TOOLTIP DA TABELA DO CAMPEONATO: como este piloto chega à etapa.
//
// A esteira de modificadores (`simulation::esteira`) sempre existiu e sempre foi invisível — o
// jogador via a chegada e nunca soube que o rival tinha largado três pontos abaixo do próprio
// número por causa de uma lesão, ou que ele mesmo estava numa boa fase. Este balão é a leitura
// dessa esteira, não uma estimativa: o backend roda a MESMA função que a corrida vai rodar.
//
// Duas colunas, e não uma: afinidade de pista entra na classificação multiplicada, e o acerto
// de fim de semana é sorteado por trim. O piloto que voa no sábado e some no domingo é um
// personagem real do modelo, e um número só o apagaria.
//
// O ícone não é enfeite: a lista é lida de relance, no meio de um hover que dura o tempo de
// decidir a próxima etapa. A forma da linha é reconhecida antes do texto ser lido — e é o que
// permite achar "lesão" na lista sem varrer sete rótulos.
//
// OS OITO ELOS APARECEM SEMPRE, na ordem em que a esteira os aplica, mesmo valendo zero. Duas
// razões: uma lista de tamanho variável obriga a reler tudo a cada piloto, e — mais importante —
// zero é informação. "Este piloto não está lesionado" e "este piloto não tem linha de lesão"
// são a mesma tela quando se esconde o zero, e só a primeira é verdade.

export const LARGURA = 300;

const VERDE = "#3fb950";
const VERMELHO = "#f85149";
const NEUTRO = "#8b949e";

// Abaixo disto o elo não está pegando neste fim de semana: `skill` é `u8`, então meio ponto
// pode nem chegar à pista. A linha continua na lista, apagada.
const LIMIAR_ATIVO = 0.05;

// Chave do elo (contrato com `commands::race::modificadores`) → ícone.
const ICONES = {
  trackKnowledge: Route,
  categoryAdaptation: GraduationCap,
  injury: HeartPulse,
  trackAffinity: Magnet,
  form: TrendingUp,
  setup: SlidersHorizontal,
  motivation: Flame,
  pressure: Gauge,
};

function corDoDelta(valor) {
  if (valor > LIMIAR_ATIVO) return VERDE;
  if (valor < -LIMIAR_ATIVO) return VERMELHO;
  return NEUTRO;
}

function estaAtivo(linha) {
  return Math.abs(linha.race) >= LIMIAR_ATIVO || Math.abs(linha.qualifying) >= LIMIAR_ATIVO;
}

const CLIMA = {
  damp: "modifiersWeatherDamp",
  wet: "modifiersWeatherWet",
  heavy: "modifiersWeatherHeavy",
};

// A CHUVA VEM À PARTE, e não é diagramação: ela é cobrada no score de segmento, não em pontos
// de skill, então não pode entrar no total de cima sem fazê-lo mentir.
//
// O número que decide a corrida aqui é o SEGUNDO. Na chuva o grid inteiro perde ritmo — quem é
// bom de molhado não fica rápido, fica menos lento —, então "perde 4,2" sozinho não diz se o
// piloto sobe ou desce. Contra o pelotão, diz.
function BlocoDeChuva({ chuva, idioma, t }) {
  if (!chuva) return null;
  const seco = chuva.weather === "dry";

  return (
    <div className="space-y-1 border-t border-white/10 pt-1.5" style={seco ? { opacity: 0.4 } : undefined}>
      <p className="flex items-center gap-1.5 text-[9px] font-bold uppercase tracking-[0.14em] text-[#58a6ff]">
        <CloudRain size={12} strokeWidth={2} className="shrink-0" />
        {t("nextRaceTab.labels.modifiersRainTitle")}
      </p>

      {seco ? (
        <p className="text-[10px] leading-snug text-gray-400">
          {t("nextRaceTab.labels.modifiersRainDry")}
        </p>
      ) : (
        <div className="space-y-1 text-[11px]">
          <div className="flex items-center justify-between gap-2">
            <span className="truncate text-gray-300">{t(`nextRaceTab.labels.${CLIMA[chuva.weather]}`)}</span>
            <span className="shrink-0 text-gray-400">
              {t("nextRaceTab.labels.modifiersRainSkill")}{" "}
              <span className="font-semibold tabular-nums text-white">
                {Math.round(chuva.rain_skill)}
              </span>
            </span>
          </div>
          <div className="flex items-center justify-between gap-2">
            <span className="truncate text-gray-300">{t("nextRaceTab.labels.modifiersRainLoses")}</span>
            <span className="shrink-0 font-semibold tabular-nums" style={{ color: VERMELHO }}>
              {formatarDelta(-chuva.penalty, idioma)}
            </span>
          </div>
          <div className="flex items-center justify-between gap-2">
            <span className="truncate font-bold text-white">
              {t("nextRaceTab.labels.modifiersRainVsField")}
            </span>
            <span
              className="shrink-0 font-bold tabular-nums"
              style={{ color: corDoDelta(chuva.vs_field) }}
            >
              {formatarDelta(chuva.vs_field, idioma)}
            </span>
          </div>
          <p className="pt-0.5 text-[9px] leading-snug text-gray-500">
            {t("nextRaceTab.labels.modifiersRainNote")}
          </p>
        </div>
      )}
    </div>
  );
}

// Uma casa decimal com sinal explícito. O sinal é o que se lê primeiro numa lista de
// modificadores — sem ele, "1,4" e "-1,4" custam o mesmo esforço de leitura.
function formatarDelta(valor, idioma) {
  const arredondado = Math.abs(valor) < 0.05 ? 0 : valor;
  const numero = Math.abs(arredondado).toLocaleString(idioma, {
    minimumFractionDigits: 1,
    maximumFractionDigits: 1,
  });
  if (arredondado > 0) return `+${numero}`;
  if (arredondado < 0) return `−${numero}`;
  return numero;
}

function Linha({ Icone, rotulo, corrida, classificacao, idioma, forte = false, apagado = false }) {
  return (
    <div
      className="flex items-center gap-2"
      style={apagado ? { opacity: 0.32 } : undefined}
      data-inativo={apagado ? "true" : undefined}
    >
      <span className="flex min-w-0 flex-1 items-center gap-1.5">
        {Icone ? (
          <Icone size={13} strokeWidth={2} className="shrink-0" style={{ color: NEUTRO }} />
        ) : (
          <span className="w-[13px] shrink-0" />
        )}
        <span className={`min-w-0 truncate ${forte ? "font-bold text-white" : "text-gray-300"}`}>
          {rotulo}
        </span>
      </span>
      <span
        className="w-10 shrink-0 text-right font-semibold tabular-nums"
        style={{ color: corDoDelta(corrida) }}
      >
        {formatarDelta(corrida, idioma)}
      </span>
      <span
        className="w-10 shrink-0 text-right tabular-nums"
        style={{ color: corDoDelta(classificacao), opacity: 0.7 }}
      >
        {formatarDelta(classificacao, idioma)}
      </span>
    </div>
  );
}

export function WeekendModifiersTip({ driverName, modifiers }) {
  const { t, i18n } = useTranslation();
  const idioma = i18n.language;
  const linhas = modifiers?.modifiers ?? [];

  return (
    <div className="w-[276px] space-y-2 py-0.5">
      <div>
        <p className="truncate text-[12px] font-bold leading-tight text-white">{driverName}</p>
        <p className="text-[9px] font-bold uppercase tracking-[0.14em] text-[#58a6ff]">
          {t("nextRaceTab.labels.modifiersTitle")}
        </p>
      </div>

      {linhas.length === 0 ? (
        <p className="text-[11px] leading-snug text-gray-400">
          {t("nextRaceTab.labels.modifiersEmpty")}
        </p>
      ) : (
        <div className="space-y-1.5 text-[11px]">
          <div className="flex items-center gap-2 text-[8px] font-bold uppercase tracking-[0.1em] text-gray-500">
            <span className="min-w-0 flex-1" />
            <span className="w-10 shrink-0 text-right">
              {t("nextRaceTab.labels.modifiersColumnRace")}
            </span>
            <span className="w-10 shrink-0 text-right">
              {t("nextRaceTab.labels.modifiersColumnQualifying")}
            </span>
          </div>

          {linhas.map((linha) => (
            <Linha
              key={linha.key}
              Icone={ICONES[linha.key]}
              rotulo={t(
                `nextRaceTab.labels.modifiers${linha.key.charAt(0).toUpperCase()}${linha.key.slice(1)}`,
              )}
              corrida={linha.race}
              classificacao={linha.qualifying}
              idioma={idioma}
              apagado={!estaAtivo(linha)}
            />
          ))}

          <div className="border-t border-white/10 pt-1.5">
            <Linha
              rotulo={t("nextRaceTab.labels.modifiersBalance")}
              corrida={modifiers.total_race}
              classificacao={modifiers.total_qualifying}
              idioma={idioma}
              forte
            />
          </div>

          <p className="pt-0.5 text-[9px] leading-snug text-gray-500">
            {t("nextRaceTab.labels.modifiersFootnote")}
          </p>
        </div>
      )}

      <BlocoDeChuva chuva={modifiers?.rain} idioma={idioma} t={t} />
    </div>
  );
}

export default WeekendModifiersTip;
