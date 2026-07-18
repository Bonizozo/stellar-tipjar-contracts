import {
  CATEGORIES,
  CATEGORY_MAP,
  ALL_TAGS,
  getCategoryForTag,
  filterByCategory,
  filterByTags,
  isValidCategoryId,
  isValidTag,
} from '../categories';

describe('CATEGORIES', () => {
  it('has at least one category', () => {
    expect(CATEGORIES.length).toBeGreaterThan(0);
  });

  it('every category has a non-empty id, label, and tags', () => {
    for (const cat of CATEGORIES) {
      expect(cat.id).toBeTruthy();
      expect(cat.label).toBeTruthy();
      expect(cat.tags.length).toBeGreaterThan(0);
    }
  });
});

describe('CATEGORY_MAP', () => {
  it('contains all category IDs', () => {
    for (const cat of CATEGORIES) {
      expect(CATEGORY_MAP[cat.id]).toBeDefined();
    }
  });
});

describe('ALL_TAGS', () => {
  it('is non-empty', () => {
    expect(ALL_TAGS.length).toBeGreaterThan(0);
  });

  it('contains tags from all categories', () => {
    for (const cat of CATEGORIES) {
      for (const tag of cat.tags) {
        expect(ALL_TAGS).toContain(tag);
      }
    }
  });
});

describe('getCategoryForTag', () => {
  it('returns the correct category for a known tag', () => {
    const cat = getCategoryForTag('musician');
    expect(cat?.id).toBe('music');
  });

  it('returns undefined for an unknown tag', () => {
    expect(getCategoryForTag('unknown-tag-xyz')).toBeUndefined();
  });
});

describe('filterByCategory', () => {
  const creators = [
    { address: 'A', categoryId: 'music' },
    { address: 'B', categoryId: 'art' },
    { address: 'C', categoryId: 'music' },
  ];

  it('filters by category ID', () => {
    const result = filterByCategory(creators, 'music');
    expect(result).toHaveLength(2);
    expect(result.every((c) => c.categoryId === 'music')).toBe(true);
  });

  it('returns all creators when categoryId is null', () => {
    expect(filterByCategory(creators, null)).toHaveLength(3);
  });

  it('returns empty array when no match', () => {
    expect(filterByCategory(creators, 'gaming')).toHaveLength(0);
  });
});

describe('filterByTags', () => {
  const creators = [
    { address: 'A', tags: ['musician', 'producer'] },
    { address: 'B', tags: ['illustrator', 'designer'] },
    { address: 'C', tags: ['musician', 'blogger'] },
  ];

  it('filters by a single tag', () => {
    const result = filterByTags(creators, ['musician']);
    expect(result).toHaveLength(2);
  });

  it('uses OR logic for multiple tags', () => {
    const result = filterByTags(creators, ['illustrator', 'blogger']);
    expect(result).toHaveLength(2);
  });

  it('returns all creators when tags array is empty', () => {
    expect(filterByTags(creators, [])).toHaveLength(3);
  });
});

describe('isValidCategoryId', () => {
  it('returns true for known category', () => {
    expect(isValidCategoryId('music')).toBe(true);
    expect(isValidCategoryId('tech')).toBe(true);
  });

  it('returns false for unknown category', () => {
    expect(isValidCategoryId('unknown')).toBe(false);
  });
});

describe('isValidTag', () => {
  it('returns true for known tag', () => {
    expect(isValidTag('musician')).toBe(true);
    expect(isValidTag('open-source')).toBe(true);
  });

  it('returns false for unknown tag', () => {
    expect(isValidTag('not-a-real-tag')).toBe(false);
  });
});
