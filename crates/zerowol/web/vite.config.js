import { defineConfig } from "vite";
import vue from "@vitejs/plugin-vue";

// The minifier drops the bundled libraries' license comments; keep one notice.
const NOTICE =
  "/*! zerowol | bundles Vue (MIT, (c) Evan You), Framework7 and Framework7-Vue (MIT, (c) Vladimir Kharlampidi) */\n";
const notice = {
  name: "zerowol-notice",
  enforce: "post",
  generateBundle(_, bundle) {
    for (const f of Object.values(bundle)) {
      if (f.type === "chunk") f.code = NOTICE + f.code;
      else if (f.fileName.endsWith(".css")) f.source = NOTICE + f.source;
    }
  },
};

// Builds src/main.js into two fixed-name files, dist/app.js and dist/app.css,
// which are committed and embedded into the binary by src/assets.rs.
export default defineConfig({
  plugins: [vue(), notice],
  publicDir: false,
  // Left as a literal so assets.rs can substitute the crate version at runtime.
  define: { __VERSION__: JSON.stringify("__VERSION__") },
  build: {
    outDir: "dist",
    emptyOutDir: true,
    target: "es2019",
    cssCodeSplit: false,
    modulePreload: false,
    reportCompressedSize: false,
    chunkSizeWarningLimit: 1024, // budget enforced by scripts/web-size.mjs
    rollupOptions: {
      input: "src/main.js",
      // A classic-script IIFE is always a single chunk (index.html loads it with
      // a plain <script>), so no inlineDynamicImports is needed.
      output: {
        format: "iife",
        entryFileNames: "app.js",
        assetFileNames: "app.css",
      },
    },
  },
  server: {
    open: "/dev.html",
    proxy: {
      "/api": "http://127.0.0.1:8090",
      "/agent": "http://127.0.0.1:8090",
    },
  },
  test: { include: ["src/**/*.test.js"], passWithNoTests: true },
});
