//! Domaine `recipes` — recettes — CRUD, photos R2, import/export YAML.
//!
//! Découpage vertical : ce domaine porte ses propres couches (convention
//! partagée par tous les domaines de Week Meals, cf. ADR-0005).
//!
//! - [`domain`]         — entités, value objects, traits de repository, services purs
//! - [`application`]    — use cases : [`application::commands`] (écritures) et
//!   [`application::queries`] (lectures)
//! - [`infrastructure`] — implémentations concrètes (repos SQLx, adapters)
//! - [`presentation`]   — routes Axum, DTO
//!
//! Règle de couches (enforcée par convention) : `domain` reste pur — il
//! n'importe ni SQLx ni Axum.

pub mod application;
pub mod domain;
pub mod infrastructure;
pub mod presentation;
