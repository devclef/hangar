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
  /** Linked reference catalog model, when set. */
  catalog_model_id?: number | null;
}

export interface ModelDetail {
  model: Model;
  parts: Part[];
  /** Set when the model is linked to a reference catalog model. */
  catalog?: { catalog_model_name: string; diagram_asset: string | null } | null;
}

// Reference catalog ---------------------------------------------------------

export interface CatalogManufacturer {
  id: number;
  name: string;
  notes: string | null;
  model_count: number;
}

export interface CatalogModel {
  id: number;
  manufacturer_id: number;
  manufacturer: string;
  name: string;
  category: Category;
  /** Per-model diagram override; null = generic per-category SVG. */
  diagram_asset: string | null;
  source_file: string;
  source_checksum: string;
}

export interface CatalogPart {
  id: number;
  catalog_model_id: number;
  name: string;
  /** Official manufacturer part number; null when not verified yet. */
  part_number: string | null;
  /** Free-text grouping for the legend (e.g. "Blade grip"). */
  category: string | null;
  notes: string | null;
  /** Hotspot position on the diagram, percentages 0-100; null = not placeable. */
  diagram_x: number | null;
  diagram_y: number | null;
}

/** A catalog part with the live owned quantity (null = no linked inventory). */
export interface CatalogPartView extends CatalogPart {
  owned_quantity: number | null;
}

/** A catalog part hit from `GET /api/catalog/parts?q=…`, joined with its model. */
export interface CatalogPartSearchHit {
  id: number;
  catalog_model_id: number;
  name: string;
  part_number: string | null;
  category: string | null;
  notes: string | null;
  diagram_x: number | null;
  diagram_y: number | null;
  catalog_model_name: string;
  manufacturer: string;
  /** The catalog model's category (the part's own `category` is free text). */
  model_category: Category;
}

export interface CatalogModelDetail {
  model: CatalogModel;
  diagram_asset: string | null;
  /** User models currently linked to this catalog model. */
  linked_models: Array<{ id: number; name: string }>;
  parts: CatalogPartView[];
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
  /** Whether the "low" quantity badge may appear for this part. */
  low_stock_enabled: boolean;
  model_count?: number;
  /** '|' joined names of linked models, null when none. */
  model_names?: string | null;
  /** Linked reference catalog part, when set. */
  catalog_part_id?: number | null;
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
  /** Whether the "low" quantity badge may appear for this part. Defaults to true. */
  low_stock_enabled?: boolean;
}

/** The catalog part an inventory part is trace-linked to (part detail embed). */
export interface PartCatalogLink {
  catalog_part_id: number;
  catalog_part_name: string;
  part_number: string | null;
  catalog_model_id: number;
  catalog_model_name: string;
  manufacturer: string;
  model_category: Category;
}

export interface PartDetail {
  part: Part;
  models: Model[];
  /** Set when the part is trace-linked to a reference catalog part. */
  catalog?: PartCatalogLink | null;
}

/** One entry in the part-usage log (a repair, build, or swap). */
export interface UsageRecord {
  id: number;
  part_id: number;
  part_name: string;
  model_id: number;
  model_name: string;
  model_category: Category;
  quantity: number;
  notes: string | null;
  /** ISO date (YYYY-MM-DD) or datetime (YYYY-MM-DDTHH:MM:SS). */
  used_at: string;
}

/**
 * Bulk-edit payload. Every field is tri-state: omitted (`undefined`) leaves
 * the value untouched, `null` clears it, a value overwrites it.
 */
export interface PartBulkEdit {
  part_ids: number[];
  quantity?: number | null;
  cost?: number | null;
  vendor?: string | null;
  link?: string | null;
  photo_url?: string | null;
  notes?: string | null;
  /** Enable/disable the "low" quantity badge on every selected part. */
  low_stock_enabled?: boolean | null;
  /** Link this model to every selected part (idempotent). */
  model_id?: number;
  /** Unlink these models from every selected part (absent links are a no-op). */
  unlink_model_ids?: number[];
}

/** Payload for recording a usage; the fixed side comes from the URL. */
export interface LogUsageInput {
  quantity?: number;
  notes?: string;
  /** Optional backdate (YYYY-MM-DD or YYYY-MM-DDTHH:MM:SS); defaults to now. */
  used_at?: string;
}

/** Optional part fields the user can toggle on the "Add part" form. */
export type PartFormField =
  | 'quantity'
  | 'cost'
  | 'vendor'
  | 'link'
  | 'photo_url'
  | 'notes';

export type ThemeMode = 'system' | 'light' | 'dark';

export interface Settings {
  part_form_fields: PartFormField[];
  /** ISO-4217 code (e.g. "USD") used to display part costs. */
  currency: string;
  /** Globally enable/disable the "low quantity" badge. */
  low_stock_enabled: boolean;
  /** A part is "low" when its quantity is at or below this value. */
  low_stock_threshold: number;
  /** UI color theme; `system` follows the OS preference. */
  theme: ThemeMode;
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
  bulkEditParts(edit: PartBulkEdit): Promise<Part[]> {
    return request('/parts/bulk-edit', {
      method: 'POST',
      body: JSON.stringify(edit),
    });
  },
  logUsageForPart(partId: number, input: LogUsageInput & { model_id: number }): Promise<UsageRecord> {
    return request(`/parts/${partId}/usage`, {
      method: 'POST',
      body: JSON.stringify(input),
    });
  },
  logUsageForModel(modelId: number, input: LogUsageInput & { part_id: number }): Promise<UsageRecord> {
    return request(`/models/${modelId}/usage`, {
      method: 'POST',
      body: JSON.stringify(input),
    });
  },

  // Usage log
  listUsage(params: { part_id?: number; model_id?: number } = {}): Promise<UsageRecord[]> {
    return request(
      `/usage${qs({
        part_id: params.part_id !== undefined ? String(params.part_id) : undefined,
        model_id: params.model_id !== undefined ? String(params.model_id) : undefined,
      })}`,
    );
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
  /**
   * Trace-link an existing inventory part to a reference catalog part.
   * Returns the refreshed part detail (with the catalog summary embedded).
   */
  linkPartCatalog(partId: number, catalogPartId: number): Promise<PartDetail> {
    return request(`/parts/${partId}/link-catalog`, {
      method: 'POST',
      body: JSON.stringify({ catalog_part_id: catalogPartId }),
    });
  },
  unlinkPartCatalog(partId: number): Promise<void> {
    return request(`/parts/${partId}/link-catalog`, { method: 'DELETE' });
  },

  // Reference catalog
  listCatalogManufacturers(): Promise<CatalogManufacturer[]> {
    return request('/catalog/manufacturers');
  },
  listCatalogModels(manufacturerId: number): Promise<CatalogModel[]> {
    return request(`/catalog/manufacturers/${manufacturerId}/models`);
  },
  /**
   * Catalog model detail with live owned quantities. Pass `modelId` to scope
   * the quantities to that one user model; omit it to sum over all linked
   * models.
   */
  getCatalogModel(id: number, modelId?: number): Promise<CatalogModelDetail> {
    return request(
      `/catalog/models/${id}${modelId !== undefined ? `?model_id=${modelId}` : ''}`,
    );
  },
  linkCatalog(modelId: number, catalogModelId: number): Promise<Model> {
    return request(`/models/${modelId}/link-catalog`, {
      method: 'POST',
      body: JSON.stringify({ catalog_model_id: catalogModelId }),
    });
  },
  unlinkCatalog(modelId: number): Promise<void> {
    return request(`/models/${modelId}/link-catalog`, { method: 'DELETE' });
  },
  /**
   * One-click add of a catalog part to a model's inventory. Creates the part
   * pre-filled from the catalog entry, or increments the existing tied part's
   * quantity (delta semantics, clamped at 0). Returns the resulting part.
   */
  addToInventory(catalogPartId: number, modelId: number, quantity?: number): Promise<Part> {
    return request(`/catalog/parts/${catalogPartId}/add-to-inventory`, {
      method: 'POST',
      body: JSON.stringify({ model_id: modelId, quantity }),
    });
  },
  /**
   * Search catalog parts across all models (name / part number / notes,
   * case-insensitive). Empty query lists the first 100 parts in name order.
   */
  searchCatalogParts(q?: string): Promise<CatalogPartSearchHit[]> {
    return request(`/catalog/parts${qs({ q })}`);
  },
  /** Explicit admin deletion of a catalog part (orphan cleanup). */
  deleteCatalogPart(id: number): Promise<void> {
    return request(`/catalog/parts/${id}`, { method: 'DELETE' });
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
