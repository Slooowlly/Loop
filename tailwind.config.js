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
          secondary: '#9ba3ae',
          muted: '#484f58',
        },
        accent: {
          primary: '#58a6ff',
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
