import { defineConfig } from "oxlint";

export default defineConfig({
  plugins: ["react", "jsx-a11y", "import"],
  rules: {
    "react-hooks/exhaustive-deps": "off",
  },
});
