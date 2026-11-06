import * as fs from 'fs';
import * as path from 'path';

describe('Production Build', () => {
  it('does not contain mock.ts code in the dist bundle', () => {
    const distPath = path.resolve(__dirname, '../../dist');
    if (!fs.existsSync(distPath)) {
      console.warn('dist folder does not exist, skipping bundle check');
      return;
    }

    const assetsDir = path.join(distPath, 'assets');
    const files = fs.readdirSync(assetsDir).filter(f => f.endsWith('.js'));

    let containsMockAdapter = false;

    for (const file of files) {
      const content = fs.readFileSync(path.join(assetsDir, file), 'utf8');
      if (content.includes('axios-mock-adapter') || content.includes('mock-secret-token')) {
        containsMockAdapter = true;
        break;
      }
    }

    expect(containsMockAdapter).toBe(false);
  });
});
