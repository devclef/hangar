// Typed client for the Hangar REST API.

export type Category = 'heli' | 'plane' | 'car' | 'drone' | 'boat' | 'other';
export type ModelStatus = 'active' | 'retired' | 'sold';
export type PartSortParam = 'recent' | 'name_asc' | 'name_desc' | 'quantity_asc' | 'quantity_desc';

export interface Model {
  id: number;
  name: string;
  category: Category;
  manufacturer: string | null;
  notes: string | null;
  date_acquired: string | null;
  status: ModelStatus;
  photo_url: string | null;
  part_count?: number;
}

export interface Part {
  id: number;
  name: string;
  quantity: number;
  notes: string | null;
  link: string | null;
  photo_url: string | null;
  cost: number | null;
  vendor: string | null;
  model_count?: number;
  /** '|' joined names of linked models, null when none. */
  model_names?: string | null;
}

export interface ModelInput {
  name: string;
  category: Category;
  manufacturer: string | null;
  notes: string | null;
  date_acquired: string | null;
  status: ModelStatus;
  photo_url: string | null;
}

export interface PartInput {
  name: string;
  quantity: number;
  notes: string | null;
  link: string | null;
  photo_url: string | null;
  cost: number | null;
  vendor: string | null;
}

export interface ModelDetail {
  model: Model;
  parts: Part[];
}

export interface PartDetail {
  part: Part;
  models: Model[];
}

/** Optional part fields the user can toggle on the "Add part" form. */
export type PartFormField =
  | 'quantity'
  | 'cost'
  | 'vendor'
  | 'link'
  | 'photo_url'
  | 'notes';

export interface Settings {
  part_form_fields: PartFormField[];
  /** ISO-4217 code (e.g. "USD") used to display part costs. */
  currency: string;
}

/** Labels for the toggleable part fields, in the order the form lays them out. */
export const PART_FORM_FIELDS: Array<{ key: PartFormField; label: string; hint: string }> = [
  { key: 'quantity', label: 'Quantity on hand', hint: 'How many you currently have' },
  { key: 'cost', label: 'Cost', hint: 'What you paid, per unit' },
  { key: 'vendor', label: 'Vendor', hint: 'Where you got it from' },
  { key: 'link', label: 'Link / SKU', hint: 'Product page URL or stock-keeping number' },
  { key: 'photo_url', label: 'Photo URL', hint: 'Image of the part' },
  { key: 'notes', label: 'Notes', hint: 'Size, pitch, material, anything else' },
];

export class ApiError extends Error {
  constructor(
    message: string,
    public readonly status: number,
  ) {
    super(message);
  }
}

async function request<T>(path: string, init?: RequestInit): Promise<T> {
  const res = await fetch(`/api${path}`, {
    headers: { 'Content-Type': 'application/json' },
    ...init,
  });
  if (!res.ok) {
    let message = `${res.status} ${res.statusText}`;
    try {
      const body = (await res.json()) as { message?: string };
      if (body.message) message = body.message;
    } catch {
      // non-JSON error body; keep the default message
    }
    throw new ApiError(message, res.status);
  }
  if (res.status === 204) return undefined as T;
  return (await res.json()) as T;
}

const qs = (params: Record<string, string | undefined>) => {
  const entries = Object.entries(params).filter(
    (entry): entry is [string, string] => entry[1] !== undefined && entry[1] !== '',
  );
  return entries.length ? `?${new URLSearchParams(entries).toString()}` : '';
};

export const api = {
  // Models
  listModels(params: { q?: string; category?: Category } = {}): Promise<Model[]> {
    return request(`/models${qs({ q: params.q, category: params.category })}`);
  },
  getModel(id: number): Promise<ModelDetail> {
    return request(`/models/${id}`);
  },
  createModel(input: ModelInput): Promise<Model> {
    return request('/models', { method: 'POST', body: JSON.stringify(input) });
  },
  updateModel(id: number, input: ModelInput): Promise<Model> {
    return request(`/models/${id}`, { method: 'PUT', body: JSON.stringify(input) });
  },
  deleteModel(id: number): Promise<void> {
    return request(`/models/${id}`, { method: 'DELETE' });
  },
  listModelParts(id: number): Promise<Part[]> {
    return request(`/models/${id}/parts`);
  },
  linkPart(modelId: number, partId: number): Promise<void> {
    return request(`/models/${modelId}/parts`, {
      method: 'POST',
      body: JSON.stringify({ part_id: partId }),
    });
  },
  unlinkPart(modelId: number, partId: number): Promise<void> {
    return request(`/models/${modelId}/parts/${partId}`, { method: 'DELETE' });
  },
  replaceModelParts(modelId: number, partIds: number[]): Promise<Part[]> {
    return request(`/models/${modelId}/parts`, {
      method: 'PUT',
      body: JSON.stringify({ part_ids: partIds }),
    });
  },

  // Parts
  listParts(
    params: { q?: string; sort?: PartSortParam } = {},
  ): Promise<Part[]> {
    return request(
      `/parts${qs({ q: params.q, sort: params.sort })}`,
    );
  },
  getPart(id: number): Promise<PartDetail> {
    return request(`/parts/${id}`);
  },
  createPart(input: PartInput): Promise<Part> {
    return request('/parts', { method: 'POST', body: JSON.stringify(input) });
  },
  updatePart(id: number, input: PartInput): Promise<Part> {
    return request(`/parts/${id}`, { method: 'PUT', body: JSON.stringify(input) });
  },
  deletePart(id: number): Promise<void> {
    return request(`/parts/${id}`, { method: 'DELETE' });
  },
  adjustQuantity(id: number, delta: number): Promise<Part> {
    return request(`/parts/${id}/quantity`, {
      method: 'POST',
      body: JSON.stringify({ delta }),
    });
  },
  listPartModels(id: number): Promise<Model[]> {
    return request(`/parts/${id}/models`);
  },
  linkModel(partId: number, modelId: number): Promise<void> {
    return request(`/parts/${partId}/models`, {
      method: 'POST',
      body: JSON.stringify({ model_id: modelId }),
    });
  },
  unlinkModel(partId: number, modelId: number): Promise<void> {
    return request(`/parts/${partId}/models/${modelId}`, { method: 'DELETE' });
  },

  // Settings
  getSettings(): Promise<Settings> {
    return request('/settings');
  },
  updateSettings(settings: Settings): Promise<Settings> {
    return request('/settings', { method: 'PUT', body: JSON.stringify(settings) });
  },
};

export const errorMessage = (e: unknown): string =>
  e instanceof Error ? e.message : 'Something went wrong';
