import js from "@eslint/js";
import prettier from "eslint-config-prettier";
import importX from "eslint-plugin-import-x";
import pluginPrettier from "eslint-plugin-prettier";
import reactHooks from "eslint-plugin-react-hooks";
import reactRefresh from "eslint-plugin-react-refresh";
import security from "eslint-plugin-security";
import sonarjs from "eslint-plugin-sonarjs";
import globals from "globals";
import tseslint from "typescript-eslint";

export default tseslint.config(
  { ignores: ["dist"] },
  {
    files: ["**/*.{ts,tsx}"],
    extends: [
      js.configs.recommended,
      ...tseslint.configs.strict,
      ...tseslint.configs.stylistic,
      sonarjs.configs.recommended,
      security.configs.recommended,
      reactHooks.configs.flat.recommended,
      reactRefresh.configs.vite,
    ],
    languageOptions: {
      ecmaVersion: 2022,
      globals: globals.browser,
    },
    plugins: {
      "import-x": importX,
      prettier: pluginPrettier,
    },
    rules: {
      // Disable rules that conflict with Prettier
      ...prettier.rules,

      // Prettier as ESLint rule
      "prettier/prettier": "error",

      // Strict JS rules
      "no-console": ["error", { allow: ["warn", "error"] }],
      "no-debugger": "error",
      eqeqeq: ["error", "always"],
      "no-var": "error",
      "prefer-const": "error",

      // TypeScript strict rules
      "@typescript-eslint/no-explicit-any": "error",
      "@typescript-eslint/no-unused-vars": ["error", { argsIgnorePattern: "^_" }],
      "@typescript-eslint/consistent-type-imports": "error",
      "@typescript-eslint/no-non-null-assertion": "error",

      // Import organization (sorting handled by prettier plugin)
      "import-x/newline-after-import": "error",
      "import-x/no-duplicates": "error",

      // Relax some sonarjs rules that conflict with our patterns
      "sonarjs/cognitive-complexity": ["error", 20],
      "sonarjs/no-duplicate-string": "off",
    },
  },
  // Generated types from Rust via ts-rs use semantic type aliases (e.g. AgentId = string).
  // These are intentional for documentation and future type narrowing.
  {
    files: ["**/types/generated/**"],
    rules: {
      "sonarjs/redundant-type-aliases": "off",
    },
  },
);
