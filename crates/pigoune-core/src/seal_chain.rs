#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Link {
    pub name: &'static str,
    pub ready: bool,
}

pub const CHAIN: [Link; 8] = [
    Link {
        name: "generations of files",
        ready: false,
    },
    Link {
        name: "transactional validation",
        ready: false,
    },
    Link {
        name: "hash verification",
        ready: false,
    },
    Link {
        name: "protected previews",
        ready: false,
    },
    Link {
        name: "every way out of an image",
        ready: false,
    },
    Link {
        name: "interruption handling",
        ready: false,
    },
    Link {
        name: "previous generations handling",
        ready: false,
    },
    Link {
        name: "regression tests",
        ready: false,
    },
];

#[must_use]
pub fn sealing_available() -> bool {
    all_ready(&CHAIN)
}

#[must_use]
pub fn all_ready(chain: &[Link]) -> bool {
    chain.iter().all(|link| link.ready)
}

#[must_use]
pub fn missing(chain: &[Link]) -> Vec<&'static str> {
    chain
        .iter()
        .filter(|link| !link.ready)
        .map(|link| link.name)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sealing_needs_every_link_of_the_chain() {
        let mut chain = CHAIN;
        for link in &mut chain {
            link.ready = true;
        }
        assert!(all_ready(&chain));
        for broken in 0..chain.len() {
            let mut partial = chain;
            partial[broken].ready = false;
            assert!(!all_ready(&partial), "{}", partial[broken].name);
            assert_eq!(missing(&partial), vec![partial[broken].name]);
        }
    }

    #[test]
    fn the_chain_lists_the_eight_required_links() {
        assert_eq!(CHAIN.len(), 8);
        assert!(CHAIN.iter().all(|link| !link.name.is_empty()));
    }
}
