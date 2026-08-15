/** @type {import('tailwindcss').Config} */
export default {
  content: [
    "./index.html",
    "./src/**/*.{js,ts,jsx,tsx}",
  ],
  theme: {
    extend: {
      colors: {
        app: {
          bg: '#0E0E10',
          card: '#1C1C1E',
          'card-hover': '#2C2C2E',
          input: '#0E0E10',
          header: '#000000',
        },
        text: {
          primary: '#e6edf3',
          // Espelham as vars `--text-secondary` e `--text-muted` do `:root` em
          // index.css, que é onde o contraste foi medido e corrigido. Aqui os
          // valores tinham ficado para trás (#9ba3ae e #484f58), e como a UI
          // pinta texto pelas classes do Tailwind (`text-text-muted`) e quase
          // nunca pela var, o que o jogador via era sempre o valor velho: o
          // muted em #484f58 dá ~2:1 sobre o fundo do painel, ilegível no
          // tamanho em que ele aparece, rótulo de 10px maiúsculo e linha de
          // números em 11px mono.
          secondary: '#b6c2cf',
          muted: '#96a5b3',
        },
        accent: {
          primary: '#58a6ff',
          // O dourado do realce de seleção. Ele já existia no código como
          // `accent-secondary` em seis lugares, sem nunca ter sido declarado aqui
          // nem no `:root` — o Tailwind não gerava classe nenhuma e o realce
          // simplesmente não aparecia. O valor vem do `shadow-[inset_...rgba(242,196,109,...)]`
          // que estava cravado no mesmo elemento que pedia a classe morta.
          secondary: '#f2c46d',
          hover: '#79b8ff',
          pressed: '#388bfd',
        },
        status: {
          green: '#3fb950',
          yellow: '#d29922',
          red: '#f85149',
          orange: '#db6d28',
          purple: '#bc8cff',
        },
        border: {
          DEFAULT: '#21262d',
          hover: '#30363d',
          focus: '#58a6ff',
        },
        podium: {
          gold: '#ffd700',
          silver: '#c0c0c0',
          bronze: '#cd7f32',
        },
      },
      fontFamily: {
        sans: ['Space Grotesk Variable', 'Segoe UI', 'system-ui', 'sans-serif'],
        mono: ['Space Grotesk Variable', 'Segoe UI', 'system-ui', 'sans-serif'],
        // A monoespaçada de verdade da aba Minha Equipe, onde a leitura é de folha
        // técnica e o número precisa cair em coluna. `mono` acima NÃO é
        // monoespaçada — aponta para a mesma Space Grotesk do corpo —, e trocá-la
        // mudaria todo o app de uma vez.
        //
        // Nenhuma mono é empacotada de propósito: o alvo é Windows, onde Cascadia
        // Mono e Consolas existem sempre, e baixar um `@fontsource` a mais custaria
        // peso no bundle para um bloco de tela.
        garage: ['Cascadia Mono', 'Consolas', 'ui-monospace', 'monospace'],
      },
      fontSize: {
        'title-lg': ['18px', { lineHeight: '1.3', fontWeight: '700' }],
        'title-md': ['15px', { lineHeight: '1.3', fontWeight: '700' }],
        'title-sm': ['12px', { lineHeight: '1.3', fontWeight: '700' }],
        'body-lg': ['11px', { lineHeight: '1.5' }],
        'body': ['10px', { lineHeight: '1.5' }],
        'body-sm': ['9px', { lineHeight: '1.5' }],
      },
    },
  },
  plugins: [],
}
