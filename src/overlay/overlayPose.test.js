import {
  TARGETS,
  TARGET_KEY,
  loadDefaults,
  loadPose,
  loadRecenterKey,
  loadRecenterPad,
  loadTargetName,
  posePayload,
  poseEq,
  savePose,
} from "./overlayPose";

// A pose dos quads de VR, extraída de OverlayPositionPanel.jsx (682 linhas, zero testes na
// vistoria de 10/08/2026).
//
// O que estes casos protegem não é o cálculo — é a RECUPERAÇÃO. As poses de fábrica foram
// calibradas na mão dentro do headset, um valor de cada vez, e o painel roda também na janela
// de overlay, onde o storage pode estar bloqueado. Toda leitura daqui precisa devolver algo
// utilizável em vez de estourar: uma exceção na inicialização de um `useState` derruba a
// janela que fica por cima do jogo, no meio da corrida.

const original = { ...TARGETS.tower.factory };

beforeEach(() => {
  localStorage.clear();
});

afterEach(() => {
  // Prova que nenhum caso mutou a tabela de fábrica por engano. `loadDefaults` e `loadPose`
  // espalham o objeto, e um espalhamento trocado por atribuição contaminaria o padrão de
  // fábrica do processo inteiro — o botão "restaurar" passaria a restaurar a última pose.
  expect(TARGETS.tower.factory).toEqual(original);
});

describe("loadTargetName", () => {
  it("cai na torre quando não há nada salvo", () => {
    expect(loadTargetName()).toBe("tower");
  });

  it("devolve o alvo salvo", () => {
    localStorage.setItem(TARGET_KEY, "radio");
    expect(loadTargetName()).toBe("radio");
  });

  it("ignora valor fora do par conhecido", () => {
    // O nome do alvo indexa `TARGETS`; um valor solto viraria `TARGETS[undefined]` e o
    // painel abriria sem configuração nenhuma.
    localStorage.setItem(TARGET_KEY, "capacete");
    expect(loadTargetName()).toBe("tower");
  });
});

describe("loadDefaults", () => {
  it("devolve a pose de fábrica quando o jogador nunca fixou um padrão", () => {
    expect(loadDefaults(TARGETS.tower)).toEqual(TARGETS.tower.factory);
  });

  it("devolve uma CÓPIA, não a tabela de fábrica", () => {
    const d = loadDefaults(TARGETS.radio);
    expect(d).not.toBe(TARGETS.radio.factory);
    d.y = 99;
    expect(TARGETS.radio.factory.y).toBe(0.22);
  });

  it("mescla o padrão salvo sobre o de fábrica, preservando campo que o salvo não tem", () => {
    // Um padrão gravado por versão antiga do painel não tem `pitch` (o eixo entrou depois).
    // Substituir em vez de mesclar deixaria a pose com buraco justo no eixo sem slider.
    localStorage.setItem(TARGETS.tower.defaultKey, JSON.stringify({ y: 1.2, scale: 2 }));
    expect(loadDefaults(TARGETS.tower)).toEqual({
      ...TARGETS.tower.factory,
      y: 1.2,
      scale: 2,
    });
  });

  it("cai na fábrica quando o padrão salvo está corrompido", () => {
    localStorage.setItem(TARGETS.tower.defaultKey, "{isso não é json");
    expect(loadDefaults(TARGETS.tower)).toEqual(TARGETS.tower.factory);
  });
});

describe("loadPose", () => {
  it("empilha fábrica, padrão do jogador e pose salva, nesta ordem", () => {
    localStorage.setItem(TARGETS.radio.defaultKey, JSON.stringify({ y: 0.5, scale: 0.4 }));
    localStorage.setItem(TARGETS.radio.poseKey, JSON.stringify({ y: 0.9 }));
    const pose = loadPose(TARGETS.radio);
    expect(pose.y).toBe(0.9); // da pose salva
    expect(pose.scale).toBe(0.4); // do padrão do jogador
    expect(pose.z).toBe(TARGETS.radio.factory.z); // da fábrica
  });

  it("cai no padrão quando a pose salva está corrompida", () => {
    localStorage.setItem(TARGETS.radio.defaultKey, JSON.stringify({ y: 0.5 }));
    localStorage.setItem(TARGETS.radio.poseKey, "não é json");
    expect(loadPose(TARGETS.radio).y).toBe(0.5);
  });

  it("os dois alvos não se contaminam", () => {
    // Torre e rádio compartilham a forma da pose e o mesmo painel. Chaves cruzadas fariam
    // ajustar a torre mover o rádio junto.
    localStorage.setItem(TARGETS.tower.poseKey, JSON.stringify({ y: 5 }));
    expect(loadPose(TARGETS.tower).y).toBe(5);
    expect(loadPose(TARGETS.radio).y).toBe(TARGETS.radio.factory.y);
  });
});

describe("loadRecenterKey", () => {
  it("devolve null sem nada salvo", () => {
    expect(loadRecenterKey(TARGETS.tower)).toBeNull();
  });

  it("aceita uma tecla válida", () => {
    localStorage.setItem(TARGETS.tower.recenterKeyStore, JSON.stringify({ vk: 82, label: "R" }));
    expect(loadRecenterKey(TARGETS.tower)).toEqual({ vk: 82, label: "R" });
  });

  it("trata vk 0 como desligada", () => {
    // 0 é o valor que o app manda ao Rust para DESLIGAR o recentro. Devolvê-lo como tecla
    // faria o painel mostrar um atalho que não existe.
    localStorage.setItem(TARGETS.tower.recenterKeyStore, JSON.stringify({ vk: 0 }));
    expect(loadRecenterKey(TARGETS.tower)).toBeNull();
  });

  it("descarta forma inesperada", () => {
    localStorage.setItem(TARGETS.tower.recenterKeyStore, JSON.stringify({ vk: "R" }));
    expect(loadRecenterKey(TARGETS.tower)).toBeNull();
    localStorage.setItem(TARGETS.tower.recenterKeyStore, "lixo");
    expect(loadRecenterKey(TARGETS.tower)).toBeNull();
  });
});

describe("loadRecenterPad", () => {
  it("aceita dispositivo e botão ZERO", () => {
    // O primeiro botão do primeiro volante é (0, 0). Um teste por verdade em vez de por tipo
    // descartaria exatamente essa combinação — a mais comum de todas.
    localStorage.setItem(TARGETS.radio.recenterPadStore, JSON.stringify({ dispositivo: 0, botao: 0 }));
    expect(loadRecenterPad(TARGETS.radio)).toEqual({ dispositivo: 0, botao: 0 });
  });

  it("exige inteiro nos dois campos", () => {
    localStorage.setItem(TARGETS.radio.recenterPadStore, JSON.stringify({ dispositivo: 0, botao: 1.5 }));
    expect(loadRecenterPad(TARGETS.radio)).toBeNull();
    localStorage.setItem(TARGETS.radio.recenterPadStore, JSON.stringify({ dispositivo: "0", botao: 1 }));
    expect(loadRecenterPad(TARGETS.radio)).toBeNull();
  });
});

describe("savePose", () => {
  it("grava na chave do alvo e volta pela leitura", () => {
    const pose = { ...TARGETS.tower.factory, y: 1.11 };
    savePose(TARGETS.tower, pose);
    expect(loadPose(TARGETS.tower)).toEqual(pose);
  });

  it("engole falha de storage em vez de interromper o ajuste ao vivo", () => {
    const original = Storage.prototype.setItem;
    Storage.prototype.setItem = () => {
      throw new Error("QuotaExceededError");
    };
    try {
      expect(() => savePose(TARGETS.tower, TARGETS.tower.factory)).not.toThrow();
    } finally {
      Storage.prototype.setItem = original;
    }
  });
});

describe("poseEq", () => {
  const base = { ...TARGETS.tower.factory };

  it("tolera a perda de precisão da ida e volta em f32", () => {
    // A layer devolve f32 e o app guarda f64. Comparar por igualdade exata faria o painel
    // achar que a pose mudou a cada poll e reescrever para sempre.
    expect(poseEq(base, { ...base, y: base.y + 1e-6 })).toBe(true);
  });

  it("enxerga uma diferença que o jogador consegue ver", () => {
    expect(poseEq(base, { ...base, y: base.y + 0.01 })).toBe(false);
    expect(poseEq(base, { ...base, scale: base.scale + 0.05 })).toBe(false);
  });

  it("compara trava e visibilidade por igualdade exata", () => {
    expect(poseEq(base, { ...base, lockMode: 0 })).toBe(false);
    expect(poseEq(base, { ...base, visible: false })).toBe(false);
  });

  it("trata pitch ausente como zero", () => {
    // Pose salva por versão antiga não tem `pitch`. Sem o default, a comparação dava NaN e
    // `Math.abs(NaN) < 1e-3` é falso — a pose nunca parecia igual e o painel reescrevia.
    const semPitch = { ...base, pitch: undefined };
    expect(poseEq(semPitch, { ...base, pitch: 0 })).toBe(true);
    expect(poseEq(semPitch, { ...base, pitch: 30 })).toBe(false);
  });

  it("é falso contra pose ausente", () => {
    expect(poseEq(null, base)).toBe(false);
    expect(poseEq(base, undefined)).toBe(false);
  });
});

describe("posePayload", () => {
  it("manda exatamente os oito campos que o comando do Rust espera", () => {
    // O comando é resolvido por string e os campos por nome; um renomeado aqui chega como
    // `undefined` no Rust, sem erro nenhum, e o quad vai parar na origem.
    expect(Object.keys(posePayload(TARGETS.tower.factory)).sort()).toEqual(
      ["lockMode", "pitch", "scale", "visible", "x", "y", "yaw", "z"],
    );
  });

  it("não carrega campo extra que a pose salva tenha acumulado", () => {
    const payload = posePayload({ ...TARGETS.tower.factory, campoVelho: 1 });
    expect(payload).not.toHaveProperty("campoVelho");
  });
});

describe("os dois alvos", () => {
  it("usam comandos e chaves de storage distintos", () => {
    const campos = ["setPose", "getPose", "recenter", "setRecenterKey", "poseKey", "defaultKey",
      "recenterKeyStore", "recenterPadStore", "alvo"];
    for (const campo of campos) {
      expect(TARGETS.tower[campo]).not.toBe(TARGETS.radio[campo]);
    }
  });

  it("declaram a pose de fábrica inteira", () => {
    // Um campo faltando na fábrica vira `undefined` no slider, que renderiza NaN e trava o
    // controle. O guard estrutural cruza estes mesmos valores com os `def_*` do Rust.
    for (const alvo of Object.values(TARGETS)) {
      for (const campo of ["lockMode", "x", "y", "z", "yaw", "pitch", "scale", "visible"]) {
        expect(alvo.factory[campo]).toBeDefined();
      }
    }
  });
});
