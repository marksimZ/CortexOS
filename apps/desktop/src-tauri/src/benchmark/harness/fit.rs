use super::extract::Requirement;
use serde_json::{Value, json};
pub fn catalog() -> Value {
    serde_json::from_str(include_str!("../../../catalogs/harness-catalog-v1.json"))
        .expect("bundled harness catalog")
}

pub fn assess(requirements: &[Requirement], weights: &Value, catalog: &Value) -> Value {
    let criteria = super::extract::criteria();
    let mut result = vec![];
    for entry in catalog["entries"].as_array().into_iter().flatten() {
        let mut yes = 0.;
        let mut unknown = 0.;
        let mut no = 0.;
        let mut hard_no = false;
        let mut hard_unknown = false;
        let mut rows = vec![];
        let mut counted = std::collections::BTreeSet::new();
        for r in requirements
            .iter()
            .filter(|r| r.status != "rejected" && !r.value.is_empty())
        {
            let fact = r.criterion.as_deref().and_then(|k| entry["facts"].get(k));
            let group = r
                .criterion
                .as_deref()
                .and_then(|k| criteria[k]["group"].as_str())
                .unwrap_or("context");
            // Repeated mentions and context facts do not multiply the matching score.
            let weight = if let Some(key) = r.criterion.as_deref() {
                if counted.insert(key) {
                    weights[group].as_u64().unwrap_or(3) as f64
                } else {
                    0.
                }
            } else {
                0.
            };
            let state = match fact.and_then(|f| f["value"].as_bool()) {
                Some(true) => {
                    yes += weight;
                    "documented"
                }
                Some(false) => {
                    no += weight;
                    hard_no |= r.hard;
                    "incompatible"
                }
                None => {
                    unknown += weight;
                    hard_unknown |= r.hard;
                    "unknown"
                }
            };
            rows.push(json!({"requirementId":r.id,"weight":weight,"state":state,"fact":fact,"provisional":r.status=="candidate"}));
        }
        let total = yes + unknown + no;
        result.push(json!({"harnessId":entry["id"],"eligibility":if hard_no{"excluded"}else if hard_unknown{"needs_verification"}else{"document_only"},
   "lowerPercent":(total>0.).then(||100.*yes/total),"upperPercent":(total>0.).then(||100.*(yes+unknown)/total),
   "evidenceCoverage":(total>0.).then(||100.*(yes+no)/total),"weights":{"supported":yes,"unknown":unknown,"incompatible":no,"total":total},"verifiedCount":0,"rows":rows}));
    }
    json!({"kind":"Document Fit","engineVersion":"document-fit-1","method":"weighted_documented_support_bounds; unknown_never_implies_compatibility; no_runtime_performance", "results":result})
}
