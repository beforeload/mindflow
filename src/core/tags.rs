use std::collections::{BTreeMap, BTreeSet};

use super::model::Memo;

#[derive(Clone, Debug)]
pub struct TagNode {
    pub name: String,
    pub path: String,
    pub count: usize,
    pub children: Vec<TagNode>,
}

/// Every prefix of a hierarchical tag: `a/b/c` yields `a`, `a/b`, `a/b/c`.
pub fn tag_prefixes(tag: &str) -> impl Iterator<Item = &str> {
    tag.match_indices('/')
        .map(move |(i, _)| &tag[..i])
        .chain(std::iter::once(tag))
}

pub fn tag_counts<'a>(memos: impl Iterator<Item = &'a Memo>) -> BTreeMap<String, usize> {
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for memo in memos {
        let mut seen: BTreeSet<&str> = BTreeSet::new();
        for tag in &memo.tags {
            for prefix in tag_prefixes(tag) {
                seen.insert(prefix);
            }
        }
        for path in seen {
            *counts.entry(path.to_string()).or_default() += 1;
        }
    }
    counts
}

pub fn build_tag_tree(counts: &BTreeMap<String, usize>) -> Vec<TagNode> {
    fn insert(nodes: &mut Vec<TagNode>, parts: &[&str], path_so_far: String, count: usize) {
        let name = parts[0];
        let path = if path_so_far.is_empty() {
            name.to_string()
        } else {
            format!("{path_so_far}/{name}")
        };
        let idx = match nodes.iter().position(|n| n.name == name) {
            Some(i) => i,
            None => {
                nodes.push(TagNode {
                    name: name.to_string(),
                    path: path.clone(),
                    count: 0,
                    children: Vec::new(),
                });
                nodes.len() - 1
            }
        };
        if parts.len() == 1 {
            nodes[idx].count = count;
        } else {
            insert(&mut nodes[idx].children, &parts[1..], path, count);
        }
    }

    let mut roots = Vec::new();
    for (path, count) in counts {
        let parts: Vec<&str> = path.split('/').filter(|p| !p.is_empty()).collect();
        if parts.is_empty() {
            continue;
        }
        insert(&mut roots, &parts, String::new(), *count);
    }
    roots
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prefixes() {
        let p: Vec<&str> = tag_prefixes("a/b/c").collect();
        assert_eq!(p, vec!["a", "a/b", "a/b/c"]);
    }
}
