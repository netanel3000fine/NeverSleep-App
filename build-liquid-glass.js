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

  // 1. Copy official styles.css and append custom Never Sleep enhancements
  if (fs.existsSync(cssSource)) {
    let cssContent = fs.readFileSync(cssSource, 'utf8');
    const customOverrides = `
/* Custom Never Sleep enhancements: High-contrast switch toggle */
.lg-switch {
  border-radius: 9999px !important;
  box-shadow: 0 0 0 1px rgba(255, 255, 255, 0.32), 0 2px 6px rgba(0, 0, 0, 0.35) !important;
  transition: box-shadow 0.2s ease, background-color 0.2s ease !important;
}

.lg-switch-track {
  border-radius: 9999px !important;
  background: rgba(0, 0, 0, 0.45) !important;
  border: 1px solid rgba(255, 255, 255, 0.22) !important;
  box-shadow: inset 0 1.5px 3px rgba(0, 0, 0, 0.45) !important;
  transition: background-color 0.2s ease, border-color 0.2s ease !important;
}

.lg-switch[data-checked] > .lg-switch-track,
.lg-switch[data-presentation-checked="true"] > .lg-switch-track {
  background: var(--lg-accent) !important;
  border-color: rgba(255, 255, 255, 0.35) !important;
  box-shadow: inset 0 1px 1.5px rgba(255, 255, 255, 0.4), 0 0 10px var(--lg-accent) !important;
}

.lg-switch[data-presentation-checked="false"] > .lg-switch-track {
  background: rgba(0, 0, 0, 0.45) !important;
  border-color: rgba(255, 255, 255, 0.22) !important;
}

body.white-mode .lg-switch {
  box-shadow: 0 0 0 1px rgba(0, 0, 0, 0.22), 0 2px 5px rgba(0, 0, 0, 0.08) !important;
}

body.white-mode .lg-switch-track {
  background: rgba(0, 0, 0, 0.14) !important;
  border: 1px solid rgba(0, 0, 0, 0.2) !important;
  box-shadow: inset 0 1px 2px rgba(0, 0, 0, 0.15) !important;
}

body.white-mode .lg-switch[data-checked] > .lg-switch-track,
body.white-mode .lg-switch[data-presentation-checked="true"] > .lg-switch-track {
  background: var(--lg-accent) !important;
  border-color: rgba(0, 0, 0, 0.25) !important;
  box-shadow: inset 0 1px 1px rgba(255, 255, 255, 0.5), 0 0 8px var(--lg-accent) !important;
}

body.white-mode .lg-switch[data-presentation-checked="false"] > .lg-switch-track {
  background: rgba(0, 0, 0, 0.14) !important;
  border-color: rgba(0, 0, 0, 0.2) !important;
}
`;
    cssContent += customOverrides;
    fs.writeFileSync(outCss, cssContent, 'utf8');
    fs.writeFileSync(outDistCss, cssContent, 'utf8');
    console.log('Copied liquid-glass.css with enhancements -> root & dist/');
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

  // 3. Sync sounds directory to dist/sounds
  const soundsSrc = path.join(rootDir, 'sounds');
  const distSounds = path.join(rootDir, 'dist', 'sounds');
  if (fs.existsSync(soundsSrc)) {
    if (!fs.existsSync(distSounds)) {
      fs.mkdirSync(distSounds, { recursive: true });
    }
    const soundFiles = fs.readdirSync(soundsSrc);
    let copiedCount = 0;
    for (const file of soundFiles) {
      fs.copyFileSync(path.join(soundsSrc, file), path.join(distSounds, file));
      copiedCount++;
    }
    console.log(`Synced ${copiedCount} sound files -> dist/sounds/`);
  }

  const stats = fs.statSync(outJs);
  console.log(`Liquid Glass bundle built successfully! Size: ${Math.round(stats.size / 1024)} KB`);
} catch (err) {
  console.error('Liquid Glass build failed:', err);
  process.exit(1);
}
