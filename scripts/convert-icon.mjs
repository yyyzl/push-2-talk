import sharp from 'sharp';
import pngToIco from 'png-to-ico';
import { promises as fs } from 'fs';
import path from 'path';
import { fileURLToPath } from 'url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const iconsDir = path.join(__dirname, '..', 'src-tauri', 'icons');
const svgPath = path.join(iconsDir, 'icon.svg');

async function convert() {
  const svg = await fs.readFile(svgPath);

  // Generate PNGs
  const sizes = [32, 128, 256];
  for (const size of sizes) {
    const name = size === 256 ? '128x128@2x.png' : `${size}x${size}.png`;
    await sharp(svg).resize(size, size).png().toFile(path.join(iconsDir, name));
    console.log(`Created ${name}`);
  }

  // Generate proper ICO with multiple sizes
  const icoBuffer = await pngToIco([
    path.join(iconsDir, '32x32.png'),
    path.join(iconsDir, '128x128.png'),
    path.join(iconsDir, '128x128@2x.png')
  ]);
  await fs.writeFile(path.join(iconsDir, 'icon.ico'), icoBuffer);
  console.log('Created icon.ico');
}

convert().catch(console.error);
