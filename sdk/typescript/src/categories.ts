/**
 * Creator category and tag definitions for the TipJar platform.
 *
 * Categories are broad groupings; tags are fine-grained labels within a category.
 */

export interface Category {
  id: string;
  label: string;
  description: string;
  tags: string[];
}

/** All supported creator categories. */
export const CATEGORIES: Category[] = [
  {
    id: 'music',
    label: 'Music',
    description: 'Musicians, producers, DJs, and audio creators',
    tags: ['musician', 'producer', 'dj', 'singer', 'songwriter', 'beatmaker', 'podcast'],
  },
  {
    id: 'art',
    label: 'Art & Design',
    description: 'Visual artists, illustrators, designers, and photographers',
    tags: ['illustrator', 'designer', 'photographer', 'painter', 'sculptor', 'digital-art', 'nft'],
  },
  {
    id: 'writing',
    label: 'Writing',
    description: 'Writers, bloggers, journalists, and storytellers',
    tags: ['blogger', 'journalist', 'novelist', 'poet', 'newsletter', 'technical-writer'],
  },
  {
    id: 'gaming',
    label: 'Gaming',
    description: 'Streamers, game developers, and esports players',
    tags: ['streamer', 'game-dev', 'esports', 'speedrunner', 'modder', 'reviewer'],
  },
  {
    id: 'education',
    label: 'Education',
    description: 'Educators, tutors, and course creators',
    tags: ['tutor', 'course-creator', 'researcher', 'science', 'math', 'language', 'coding'],
  },
  {
    id: 'tech',
    label: 'Technology',
    description: 'Developers, open-source contributors, and tech creators',
    tags: ['open-source', 'developer', 'blockchain', 'ai', 'security', 'devops'],
  },
  {
    id: 'video',
    label: 'Video',
    description: 'Video creators, filmmakers, and animators',
    tags: ['youtuber', 'filmmaker', 'animator', 'vlogger', 'documentary', 'short-film'],
  },
  {
    id: 'wellness',
    label: 'Wellness',
    description: 'Fitness, mental health, and lifestyle creators',
    tags: ['fitness', 'yoga', 'meditation', 'nutrition', 'mental-health', 'lifestyle'],
  },
];

/** Map of category ID → Category for O(1) lookup. */
export const CATEGORY_MAP: Record<string, Category> = Object.fromEntries(
  CATEGORIES.map((c) => [c.id, c]),
);

/** All valid tag strings across all categories. */
export const ALL_TAGS: string[] = CATEGORIES.flatMap((c) => c.tags);

/**
 * Find the category that contains a given tag.
 * Returns undefined if the tag is not in any category.
 */
export function getCategoryForTag(tag: string): Category | undefined {
  return CATEGORIES.find((c) => c.tags.includes(tag));
}

/**
 * Filter creators by category ID.
 * @param creators - Array of creator objects with a `categoryId` field.
 * @param categoryId - Category ID to filter by. Pass `null` to return all.
 */
export function filterByCategory<T extends { categoryId?: string }>(
  creators: T[],
  categoryId: string | null,
): T[] {
  if (!categoryId) return creators;
  return creators.filter((c) => c.categoryId === categoryId);
}

/**
 * Filter creators by one or more tags (OR logic — matches any tag).
 * @param creators - Array of creator objects with a `tags` field.
 * @param tags - Tags to filter by. Empty array returns all creators.
 */
export function filterByTags<T extends { tags?: string[] }>(
  creators: T[],
  tags: string[],
): T[] {
  if (!tags.length) return creators;
  return creators.filter((c) => c.tags?.some((t) => tags.includes(t)));
}

/**
 * Validate that a category ID is known.
 */
export function isValidCategoryId(id: string): boolean {
  return id in CATEGORY_MAP;
}

/**
 * Validate that a tag string is known.
 */
export function isValidTag(tag: string): boolean {
  return ALL_TAGS.includes(tag);
}
