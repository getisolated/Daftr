import { describe, it, expect } from 'vitest';
import { MAX_GRAPH_GROUPS, graphGroups } from '../src/app/core/utils/graph-groups.util';

describe('graphGroups', () => {
  it('leaves the items at the root without a group', () => {
    expect(graphGroups(['', 'docs'])).toEqual([null, 'docs']);
  });

  it('goes down the folders while that gives few enough groups', () => {
    const folders = ['doc/install', 'doc/install', 'doc/guide', 'doc/guide/old', 'output', 'temp'];
    expect(graphGroups(folders)).toEqual(['doc/install', 'doc/install', 'doc/guide', 'doc/guide/old', 'output', 'temp']);
  });

  it('stays on a shallower level when a deeper one would give too many groups', () => {
    const many = Array.from({ length: MAX_GRAPH_GROUPS + 1 }, (_, i) => `doc/part${i}`);
    expect(new Set(graphGroups(many))).toEqual(new Set(['doc']));
  });
});
