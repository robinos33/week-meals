//! Crate `kernel` — noyau partagé, pur.
//!
//! Types transverses aux domaines : value objects communs (`Quantity`,
//! `Unit`…), identifiants (`HouseholdId`, `UserId`…) et erreurs partagées.
//!
//! `kernel` n'a aucune dépendance d'infrastructure et ne dépend d'aucun
//! domaine ; ce sont les domaines qui dépendent de `kernel`. Le contenu
//! sera ajouté au fil des jalons.
