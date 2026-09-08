use crate::{
    catalog::Catalog,
    storage::{ModReference, Origin},
};
use serde::Serialize;
use std::collections::{BTreeSet, HashMap};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Resolution {
    pub effective: Vec<ModReference>,
    pub adjustments: Vec<Adjustment>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Adjustment {
    pub before: String,
    pub after: String,
    pub message: String,
}

pub fn resolve(catalog: &Catalog, requested: &[ModReference]) -> Result<Resolution, String> {
    if requested.len() > 256 {
        return Err("This runtime supports at most 256 active mods.".into());
    }
    let mut positions = HashMap::new();
    let mut releases = Vec::new();
    let mut names = Vec::new();
    for (index, reference) in requested.iter().enumerate() {
        if positions.insert(reference.mod_id.as_str(), index).is_some() {
            return Err(format!(
                "Only one version of {} can be enabled.",
                reference.mod_id
            ));
        }
        let (owner, release) = catalog.releases().find(|(m, r)| {
            reference.origin == Origin::Catalog && m.id == reference.mod_id
                && Some(&r.id) == reference.release_id.as_ref() && r.artifact.sha256 == reference.hash
        }).ok_or_else(|| format!("Exact release metadata for {} is unavailable. Restore its catalog metadata or disable it.", reference.mod_id))?;
        releases.push(release);
        names.push(owner.name.as_str());
    }
    let mut edges = vec![BTreeSet::new(); requested.len()];
    for (index, release) in releases.iter().enumerate() {
        for required in &release.requires {
            let dependency = releases.iter().position(|r| &r.id == required)
                .ok_or_else(|| format!("{} requires release {required}. Enable the missing or disabled dependency, or disable {}.", requested[index].mod_id, requested[index].mod_id))?;
            edges[dependency].insert(index);
        }
        for (targets, before) in [(&release.load_before, true), (&release.load_after, false)] {
            for target in targets {
                if let Some(&other) = positions.get(target.as_str()) {
                    let (from, to) = if before {
                        (index, other)
                    } else {
                        (other, index)
                    };
                    edges[from].insert(to);
                }
            }
        }
    }
    if let Some(cycle) = cycle(&edges) {
        let names: Vec<_> = cycle
            .iter()
            .map(|&i| requested[i].mod_id.as_str())
            .collect();
        return Err(format!(
            "Load-order cycle: {}. Disable an involved mod or choose releases with compatible constraints.",
            names.join(" -> ")
        ));
    }
    let mut adjustments = Vec::new();
    let adjustment = |from: usize, to: usize, message| Adjustment {
        before: requested[from].mod_id.clone(),
        after: requested[to].mod_id.clone(),
        message,
    };
    for (from, targets) in edges.iter().enumerate() {
        for &to in targets {
            if from > to {
                adjustments.push(adjustment(
                    from,
                    to,
                    format!("{} must load before {}.", names[from], names[to]),
                ));
            }
        }
    }
    let mut incoming = vec![0; requested.len()];
    for targets in &edges {
        for &to in targets {
            incoming[to] += 1;
        }
    }
    let mut ready: BTreeSet<_> = incoming
        .iter()
        .enumerate()
        .filter_map(|(i, &n)| (n == 0).then_some(i))
        .collect();
    let mut effective = Vec::new();
    while let Some(index) = ready.pop_first() {
        effective.push(requested[index].clone());
        for &to in &edges[index] {
            incoming[to] -= 1;
            if incoming[to] == 0 {
                ready.insert(to);
            }
        }
    }
    let effective_positions: HashMap<_, _> = effective
        .iter()
        .enumerate()
        .map(|(i, r)| (r.mod_id.as_str(), i))
        .collect();
    for (index, release) in releases.iter().enumerate() {
        for (targets, before) in [
            (&release.prefer_before, true),
            (&release.prefer_after, false),
        ] {
            for target in targets {
                if let Some(&other) = positions.get(target.as_str()) {
                    let (from, to) = if before {
                        (index, other)
                    } else {
                        (other, index)
                    };
                    if effective_positions[requested[from].mod_id.as_str()]
                        > effective_positions[requested[to].mod_id.as_str()]
                    {
                        let note = adjustment(
                            from,
                            to,
                            format!(
                                "{} prefers to load before {}. Your priority and required constraints take precedence.",
                                names[from], names[to]
                            ),
                        );
                        if !adjustments.iter().any(|a| a.message == note.message) {
                            adjustments.push(note);
                        }
                    }
                }
            }
        }
    }
    Ok(Resolution {
        effective,
        adjustments,
    })
}

fn cycle(edges: &[BTreeSet<usize>]) -> Option<Vec<usize>> {
    fn visit(
        index: usize,
        edges: &[BTreeSet<usize>],
        states: &mut [u8],
        path: &mut Vec<usize>,
    ) -> Option<Vec<usize>> {
        if states[index] == 2 {
            return None;
        }
        if states[index] == 1 {
            let start = path.iter().position(|&i| i == index).unwrap();
            let mut result = path[start..].to_vec();
            result.push(index);
            return Some(result);
        }
        states[index] = 1;
        path.push(index);
        for &next in &edges[index] {
            if let Some(result) = visit(next, edges, states, path) {
                return Some(result);
            }
        }
        path.pop();
        states[index] = 2;
        None
    }
    let mut states = vec![0; edges.len()];
    for index in 0..edges.len() {
        if let Some(result) = visit(index, edges, &mut states, &mut Vec::new()) {
            return Some(result);
        }
    }
    None
}

#[cfg(test)]
mod tests;
