/** @type {import('tailwindcss').Config} */
export default {
  content: [
    "./index.html",
    "./src/**/*.{js,ts,jsx,tsx}",
  ],
  theme: {
    extend: {
      colors: {
        ink: "var(--ink)",
        paper: "var(--paper)",
        sand: "var(--sand)",
        stone: "var(--stone-dark)",
        crail: "var(--crail)",
        steel: "var(--steel)",
        sage: "var(--sage)",
      },
      fontFamily: {
        sans: ["Poppins", "Noto Sans SC", "Arial", "ui-sans-serif", "system-ui", "sans-serif"],
        serif: ["Lora", "Noto Serif SC", "Georgia", "ui-serif", "serif"],
        mono: ["JetBrains Mono", "ui-monospace", "SFMono-Regular", "Menlo", "Monaco", "Consolas", "monospace"],
      },
    },
  },
  plugins: [],
}
