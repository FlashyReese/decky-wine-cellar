use crate::release_util::{Release, ReleaseProvider};
use crate::wine_cask::flavors::{
    catalog_release_id, CatalogRelease, CompatibilityToolFlavor, Flavor,
};
use chrono::DateTime;
use std::cmp::Reverse;
use std::collections::{HashMap, HashSet};

pub enum ReleaseFilter {
    All,
    AssetNameContains(&'static str),
}

impl ReleaseFilter {
    fn accepts(&self, release: &Release) -> bool {
        match self {
            Self::All => true,
            Self::AssetNameContains(pattern) => release
                .assets
                .iter()
                .any(|asset| asset.name.contains(pattern)),
        }
    }
}

pub struct FlavorDefinition {
    pub flavor: CompatibilityToolFlavor,
    // The first provider wins when several providers publish the same tag.
    pub providers: &'static [ReleaseProvider],
    pub release_filter: ReleaseFilter,
}

pub const FLAVOR_DEFINITIONS: &[FlavorDefinition] = &[
    FlavorDefinition {
        flavor: CompatibilityToolFlavor::ProtonGE,
        providers: &[ReleaseProvider::github(
            "GloriousEggroll",
            "proton-ge-custom",
        )],
        release_filter: ReleaseFilter::All,
    },
    FlavorDefinition {
        flavor: CompatibilityToolFlavor::ProtonCachyOS,
        providers: &[ReleaseProvider::github("CachyOS", "proton-cachyos")],
        release_filter: ReleaseFilter::AssetNameContains("x86_64_v3"),
    },
    FlavorDefinition {
        flavor: CompatibilityToolFlavor::Luxtorpeda,
        providers: &[
            ReleaseProvider::codeberg("luxtorpeda", "luxtorpeda"),
            ReleaseProvider::github("luxtorpeda-dev", "luxtorpeda"),
        ],
        release_filter: ReleaseFilter::All,
    },
    FlavorDefinition {
        flavor: CompatibilityToolFlavor::Boxtron,
        providers: &[ReleaseProvider::github("dreamer", "boxtron")],
        release_filter: ReleaseFilter::All,
    },
];

impl FlavorDefinition {
    pub fn merge_releases<'a>(
        &self,
        provider_releases: impl IntoIterator<Item = (&'a ReleaseProvider, Vec<Release>)>,
    ) -> Flavor {
        let mut provider_releases: HashMap<_, _> = provider_releases
            .into_iter()
            .map(|(provider, releases)| (*provider, releases))
            .collect();
        let mut seen_tags = HashSet::new();
        let mut releases = Vec::new();

        for provider in self.providers {
            let source_releases = provider_releases.remove(provider).unwrap_or_default();
            for release in source_releases {
                if self.release_filter.accepts(&release)
                    && seen_tags.insert(release.tag_name.clone())
                {
                    releases.push(CatalogRelease {
                        id: catalog_release_id(&self.flavor, release.id, provider.source),
                        flavor: self.flavor.clone(),
                        release,
                    });
                }
            }
        }

        releases.sort_by_cached_key(|release| {
            Reverse(DateTime::parse_from_rfc3339(&release.release.published_at).ok())
        });

        Flavor {
            flavor: self.flavor.clone(),
            releases,
        }
    }
}
