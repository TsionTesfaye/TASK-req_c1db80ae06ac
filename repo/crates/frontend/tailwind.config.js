/** @type {import('tailwindcss').Config} */
module.exports = {
  content: [
    "./index.html",
    "./src/**/*.rs",
  ],
  theme: {
    extend: {
      colors: {
        terra: {
          50:  "#f4f7f3",
          100: "#e3ecde",
          500: "#3f6b3a",
          700: "#2b4a28",
          900: "#162814",
        },
      },
    },
  },
  plugins: [],
};
