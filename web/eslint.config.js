import js from "@eslint/js";
import globals from "globals";
import reactHooks from "eslint-plugin-react-hooks";
import reactRefresh from "eslint-plugin-react-refresh";
import tseslint from "typescript-eslint";

/**
 * Lint du front. Pendant du `clippy -D warnings` de l'API : la CI échoue sur
 * un avertissement, pour que le front tienne le même niveau d'exigence que le
 * backend (cf. ADR-0001 : « la qualité du code compte »).
 */
export default tseslint.config(
  { ignores: ["dist", "dev-dist", "coverage", "public"] },
  {
    extends: [js.configs.recommended, ...tseslint.configs.recommended],
    files: ["**/*.{ts,tsx}"],
    languageOptions: {
      ecmaVersion: 2022,
      globals: globals.browser,
    },
    plugins: {
      "react-hooks": reactHooks,
      "react-refresh": reactRefresh,
    },
    rules: {
      ...reactHooks.configs.recommended.rules,
      "react-refresh/only-export-components": [
        "warn",
        { allowConstantExport: true },
      ],
    },
  },
  {
    // Scripts Node (génération d'icônes) et config Vite.
    files: ["scripts/**/*.mjs", "*.config.{ts,js}"],
    languageOptions: { globals: globals.node },
  },
);
