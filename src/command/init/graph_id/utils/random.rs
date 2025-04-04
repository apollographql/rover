use rand::Rng;
pub trait RandomStringGenerator {
    fn generate_string(&mut self, length: usize) -> String;
}

pub struct DefaultRandomStringGenerator;

impl RandomStringGenerator for DefaultRandomStringGenerator {
    fn generate_string(&mut self, length: usize) -> String {
        let mut rng = rand::rng();
        let chars: Vec<char> = "abcdefghijklmnopqrstuvwxyz0123456789".chars().collect();
        
        (0..length)
            .map(|_| {
                let idx = rng.random_range(0..chars.len());
                chars[idx]
            })
            .collect()
    }
}

/// We're going to use this for testing because the random generator is not deterministic
#[cfg(test)]
pub struct TestRandomStringGenerator {
    pub value: String,
}

#[cfg(test)]
impl RandomStringGenerator for TestRandomStringGenerator {
    fn generate_string(&mut self, length: usize) -> String {
        let mut s = self.value.clone();
        s.truncate(length);
        let remaining = length.saturating_sub(s.len());
        s + &"a".repeat(remaining)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_random_string_generator() {
        let mut generator = DefaultRandomStringGenerator;
        let s1 = generator.generate_string(7);
        let s2 = generator.generate_string(7);
        
        // Each string should be the requested length
        assert_eq!(s1.len(), 7);
        assert_eq!(s2.len(), 7);
    }

    #[test]
    fn test_test_random_string_generator() {
        let mut generator = TestRandomStringGenerator {
            value: "testvalue".to_string(),
        };
        
        assert_eq!(generator.generate_string(4), "test");
        assert_eq!(generator.generate_string(10), "testvaluea");
    }
}