const esbuild = require('esbuild');
const fs = require('fs');
const path = require('path');

const rootDir = __dirname;
const srcFile = path.join(rootDir, 'src', 'liquid-glass-app.jsx');
const outJs = path.join(rootDir, 'liquid-glass-bundle.js');
const outDistJs = path.join(rootDir, 'dist', 'liquid-glass-bundle.js');
const cssSource = path.join(rootDir, 'node_modules', '@glass-sdk', 'liquid-glass', 'dist', 'styles.css');
const outCss = path.join(rootDir, 'liquid-glass.css');
const outDistCss = path.join(rootDir, 'dist', 'liquid-glass.css');

console.log('Building Liquid Glass React bundle...');

try {
  // Ensure dist directory exists
  if (!fs.existsSync(path.join(rootDir, 'dist'))) {
    fs.mkdirSync(path.join(rootDir, 'dist'), { recursive: true });
  }

  // 1. Copy official styles.css
  if (fs.existsSync(cssSource)) {
    fs.copyFileSync(cssSource, outCss);
    fs.copyFileSync(cssSource, outDistCss);
    console.log('Copied liquid-glass.css -> root & dist/');
  } else {
    console.warn('WARNING: @glass-sdk/liquid-glass/dist/styles.css not found yet.');
  }

  // 2. Build JS bundle with esbuild
  esbuild.buildSync({
    entryPoints: [srcFile],
    bundle: true,
    outfile: outJs,
    format: 'iife',
    globalName: 'LiquidGlassSDK',
    minify: false, // readable for debugging
    loader: { '.jsx': 'jsx', '.js': 'js', '.css': 'text' },
    define: {
      'process.env.NODE_ENV': '"production"'
    }
  });

  // Also copy to dist
  fs.copyFileSync(outJs, outDistJs);

  const stats = fs.statSync(outJs);
  console.log(`Liquid Glass bundle built successfully! Size: ${Math.round(stats.size / 1024)} KB`);
} catch (err) {
  console.error('Liquid Glass build failed:', err);
  process.exit(1);
}
