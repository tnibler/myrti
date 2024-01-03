import type { z } from 'zod';
import type { schemas } from './apiclient';

export type Asset = z.infer<typeof schemas.Asset>;
export type TimelineGroup = z.infer<typeof schemas.TimelineGroup>;
