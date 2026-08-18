// Tiny hash-based router (works with the backend's static serving,
// no server-side fallback rules needed).

export type Route =
  | { page: 'models' }
  | { page: 'model'; id: number }
  | { page: 'model-form'; id?: number }
  | { page: 'parts' }
  | { page: 'part'; id: number }
  | { page: 'part-form'; id?: number }
  | { page: 'catalog' }
  | { page: 'catalog-model'; id: number }
  | { page: 'usage' }
  | { page: 'settings' };

const toNum = (s: string | undefined): number | undefined =>
  s !== undefined && /^\d+$/.test(s) ? Number(s) : undefined;

export function parseRoute(hash: string = window.location.hash): Route {
  const seg = hash.replace(/^#\/?/, '').split('/').filter(Boolean);
  const [head, a, b] = seg;
  switch (head) {
    case undefined:
    case 'models': {
      if (a === 'new') return { page: 'model-form' };
      const id = toNum(a);
      if (id !== undefined) {
        return b === 'edit' ? { page: 'model-form', id } : { page: 'model', id };
      }
      return { page: 'models' };
    }
    case 'parts': {
      if (a === 'new') return { page: 'part-form' };
      const id = toNum(a);
      if (id !== undefined) {
        return b === 'edit' ? { page: 'part-form', id } : { page: 'part', id };
      }
      return { page: 'parts' };
    }
    case 'catalog': {
      if (a === 'models') {
        const id = toNum(b);
        if (id !== undefined) return { page: 'catalog-model', id };
      }
      return { page: 'catalog' };
    }
    case 'usage':
      return { page: 'usage' };
    case 'settings':
      return { page: 'settings' };
    default:
      return { page: 'models' };
  }
}

export const href = {
  models: '#/models',
  model: (id: number) => `#/models/${id}`,
  modelForm: (id?: number) => (id === undefined ? '#/models/new' : `#/models/${id}/edit`),
  parts: '#/parts',
  part: (id: number) => `#/parts/${id}`,
  partForm: (id?: number) => (id === undefined ? '#/parts/new' : `#/parts/${id}/edit`),
  catalog: '#/catalog',
  catalogModel: (id: number) => `#/catalog/models/${id}`,
  usage: '#/usage',
  settings: '#/settings',
};
