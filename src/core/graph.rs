//! Force-directed layout for the memo relationship graph.

use super::model::MemoId;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EdgeKind {
    Reference,
    Semantic,
}

#[derive(Clone, Debug)]
pub struct GraphNode {
    pub id: MemoId,
    pub x: f32,
    pub y: f32,
    pub degree: usize,
}

#[derive(Clone, Debug, Default)]
pub struct GraphLayout {
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<(usize, usize, EdgeKind)>,
}

/// Lays out nodes in the unit square. Edges are index pairs into `ids`.
pub fn layout(ids: Vec<MemoId>, edges: Vec<(usize, usize, EdgeKind)>) -> GraphLayout {
    let n = ids.len();
    if n == 0 {
        return GraphLayout::default();
    }
    let mut degree = vec![0usize; n];
    for &(a, b, _) in &edges {
        degree[a] += 1;
        degree[b] += 1;
    }
    let mut pos: Vec<(f32, f32)> = (0..n)
        .map(|i| {
            let angle = i as f32 * 2.399_963;
            let radius = 0.05 + 0.4 * ((i as f32 + 0.5) / n as f32).sqrt();
            (0.5 + radius * angle.cos(), 0.5 + radius * angle.sin())
        })
        .collect();
    let k = (1.0 / n as f32).sqrt() * 0.9;
    let iterations = 320;
    let mut temperature = 0.08;
    for _ in 0..iterations {
        let mut disp = vec![(0.0f32, 0.0f32); n];
        for i in 0..n {
            for j in (i + 1)..n {
                let dx = pos[i].0 - pos[j].0;
                let dy = pos[i].1 - pos[j].1;
                let d2 = (dx * dx + dy * dy).max(1e-6);
                let force = k * k / d2;
                disp[i].0 += dx * force;
                disp[i].1 += dy * force;
                disp[j].0 -= dx * force;
                disp[j].1 -= dy * force;
            }
        }
        for &(a, b, kind) in &edges {
            let dx = pos[a].0 - pos[b].0;
            let dy = pos[a].1 - pos[b].1;
            let d = (dx * dx + dy * dy).sqrt().max(1e-4);
            let strength = if kind == EdgeKind::Reference { 1.0 } else { 0.6 };
            let force = strength * d / k;
            disp[a].0 -= dx * force;
            disp[a].1 -= dy * force;
            disp[b].0 += dx * force;
            disp[b].1 += dy * force;
        }
        for i in 0..n {
            // gentle gravity keeps disconnected components on screen
            disp[i].0 += (0.5 - pos[i].0) * 0.02 / k;
            disp[i].1 += (0.5 - pos[i].1) * 0.02 / k;
            let len = (disp[i].0 * disp[i].0 + disp[i].1 * disp[i].1).sqrt().max(1e-6);
            let step = len.min(temperature);
            pos[i].0 += disp[i].0 / len * step;
            pos[i].1 += disp[i].1 / len * step;
        }
        temperature *= 0.985;
    }
    let (min_x, max_x) = pos.iter().fold((f32::MAX, f32::MIN), |(a, b), p| (a.min(p.0), b.max(p.0)));
    let (min_y, max_y) = pos.iter().fold((f32::MAX, f32::MIN), |(a, b), p| (a.min(p.1), b.max(p.1)));
    // Uniform scale with a minimum span keeps small graphs compact and centred.
    let span = (max_x - min_x).max(max_y - min_y).max(0.45);
    let (cx, cy) = ((min_x + max_x) / 2.0, (min_y + max_y) / 2.0);
    let nodes = ids
        .into_iter()
        .enumerate()
        .map(|(i, id)| GraphNode {
            id,
            x: 0.5 + 0.88 * (pos[i].0 - cx) / span,
            y: 0.5 + 0.88 * (pos[i].1 - cy) / span,
            degree: degree[i],
        })
        .collect();
    GraphLayout { nodes, edges }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn connected_nodes_are_closer() {
        let ids: Vec<String> = (0..6).map(|i| i.to_string()).collect();
        let g = layout(ids, vec![(0, 1, EdgeKind::Reference), (1, 2, EdgeKind::Reference)]);
        let d = |a: usize, b: usize| ((g.nodes[a].x - g.nodes[b].x).powi(2) + (g.nodes[a].y - g.nodes[b].y).powi(2)).sqrt();
        assert!(d(0, 1) < d(0, 5));
        assert!(g.nodes.iter().all(|n| (0.0..=1.0).contains(&n.x) && (0.0..=1.0).contains(&n.y)));
    }
}
