import sharp from 'sharp';
import pngToIco from 'png-to-ico';
import { promises as fs } from 'fs';
import { execFile } from 'node:child_process';
import { tmpdir } from 'node:os';
import { promisify } from 'node:util';
import path from 'path';
import { fileURLToPath } from 'url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const iconsDir = path.join(__dirname, '..', 'src-tauri', 'icons');

async function convert() {
  // The approved 05A mark is the single source for every platform and surface.
  const mark = await fs.readFile(path.join(iconsDir, 'mark.svg'), 'utf8');
  const paths = mark.match(/<path\b[^>]*\/>/g);
  if (paths?.length !== 3) throw new Error('mark.svg must contain the three approved 05A paths');
  const symbol = (x, y, width, fill, gap = 0) => {
    // Small tray glyphs need a little extra separation after rasterization.
    const separated = paths.map((part) => {
      const offset = part.includes('id="wave-outer"') ? gap * 2 :
        part.includes('id="wave-inner"') ? gap : 0;
      return `<g transform="translate(${offset} 0)">${part}</g>`;
    }).join('\n');
    return `<g fill="${fill}" transform="translate(${x} ${y}) scale(${width / (658 + gap * 2)})">${separated}</g>`;
  };
  const document = (width, height, title, body) =>
    `<svg xmlns="http://www.w3.org/2000/svg" width="${width}" height="${height}" viewBox="0 0 ${width} ${height}">\n<title>${title}</title>\n${body}\n</svg>\n`;

  const svg = document(1024, 1024, 'PushToTalk 05A application icon',
    '<rect x="32" y="32" width="960" height="960" rx="224" fill="#171717"/>\n' +
    symbol(122, 224, 760, '#fff'));
  // A native Mac tile needs outer breathing room so it matches its Dock neighbours.
  const macSvg = document(1024, 1024, 'PushToTalk 05A macOS application icon',
    '<rect x="100" y="100" width="824" height="824" rx="185" fill="#FAF9F5"/>\n' +
    symbol(175, 262, 650, '#171717'));
  const template = document(22, 18, 'PushToTalk 05A menu bar template',
    symbol(1, 1.8, 20, '#000', 16));
  const windowsTray = document(32, 32, 'PushToTalk 05A Windows tray icon',
    '<rect width="32" height="32" rx="7" fill="#171717"/>\n' +
    symbol(2.6, 7, 26, '#fff', 16));
  for (const [name, source] of [
    ['icon.svg', svg], ['icon-macos.svg', macSvg],
    ['tray-template.svg', template], ['tray-windows.svg', windowsTray],
  ]) {
    await fs.writeFile(path.join(iconsDir, name), source);
  }
  await fs.writeFile(path.join(__dirname, '..', 'public', 'app-icon.svg'), svg);
  const png = (source, width, height, name) => sharp(Buffer.from(source))
    .resize(width, height).png().toFile(path.join(iconsDir, name));

  // Keep every PNG referenced by tauri.conf.json in sync with the same source.
  const sizes = [32, 64, 128, 256, 512];
  for (const size of sizes) {
    const name = `${size}x${size}.png`;
    await png(size <= 32 ? windowsTray : svg, size, size, name);
    console.log(`Created ${name}`);
  }
  await png(svg, 1024, 1024, 'icon.png');
  await png(macSvg, 1024, 1024, 'icon-macos.png');
  await png(windowsTray, 32, 32, 'tray-windows.png');

  // macOS status items use an 18pt alpha template, never the application tile.
  for (const [scale, name] of [[1, 'tray-template.png'], [2, 'tray-template@2x.png']]) {
    await png(template, 22 * scale, 18 * scale, name);
    console.log(`Created ${name}`);
  }

  // Keep native small ICO frames legible, using the existing Windows encoder.
  const icoFrames = await Promise.all([16, 24, 32, 48, 64, 128, 256].map((size) =>
    sharp(Buffer.from(size <= 32 ? windowsTray : svg)).resize(size, size).png().toBuffer()));
  await fs.writeFile(path.join(iconsDir, 'icon.ico'), await pngToIco(icoFrames));

  // Let Tauri encode ICNS; don't commit its unrelated mobile/store assets.
  // Generating in a temporary directory works on both OSes.
  const temporary = await fs.mkdtemp(path.join(tmpdir(), 'ptt-icons-'));
  try {
    const cli = path.join(__dirname, '..', 'node_modules', '@tauri-apps', 'cli', 'tauri.js');
    await promisify(execFile)(process.execPath, [cli, 'icon', path.join(iconsDir, 'icon-macos.svg'), '--output', temporary]);
    await fs.copyFile(path.join(temporary, 'icon.icns'), path.join(iconsDir, 'icon.icns'));
    console.log('Created Windows ICO and macOS ICNS');
  } finally {
    await fs.rm(temporary, { recursive: true, force: true });
  }
}

convert().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
