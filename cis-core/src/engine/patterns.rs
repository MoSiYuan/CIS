//! # Injection Pattern Library
//!
//! Predefined patterns for detecting injection points in various game engines.

use super::types::{InjectionPattern, InjectionType, EngineType};
use std::sync::LazyLock;

/// Global pattern library
pub static PATTERN_LIBRARY: LazyLock<PatternLibrary> = LazyLock::new(|| {
    PatternLibrary::new()
});

/// Pattern library containing predefined injection patterns
pub struct PatternLibrary {
    /// All available patterns
    patterns: Vec<InjectionPattern>,
}

impl PatternLibrary {
    /// Create a new pattern library with default patterns
    pub fn new() -> Self {
        let mut library = Self {
            patterns: Vec::new(),
        };

        // Add default patterns for each engine
        library.add_unreal_patterns();
        library.add_unity_patterns();
        library.add_godot_patterns();
        library.add_common_patterns();

        library
    }

    /// Add Unreal Engine specific patterns
    fn add_unreal_patterns(&mut self) {
        // Unreal Function Calls
        self.patterns.push(
            InjectionPattern::new(
                "Unreal Process Call".to_string(),
                r"\b(APROJECT|CALLPROCESS|CALLPROCESSRELATIVE|CALLPROCESSCONTEXT|CALLPROCESSFUNCTION|CALLPROCESSSTRING|CALLPROCESSARRAY)\s*\("
                    .to_string(),
                InjectionType::FunctionCall,
            )
            .with_confidence(0.95)
            .with_language("C++".to_string())
            .with_description("Unreal process execution calls".to_string()),
        );

        // Unreal宏调用
        self.patterns.push(
            InjectionPattern::new(
                "Unreal Macro Call".to_string(),
                r"\b(UFUNCTION|UCLASS|USTRUCT|UPROPERTY|UENUM)\s*\("
                    .to_string(),
                InjectionType::CustomHook,
            )
            .with_confidence(1.0)
            .with_language("C++".to_string())
            .with_description("Unreal reflection macros".to_string()),
        );

        // Unreal Variable Assignment
        self.patterns.push(
            InjectionPattern::new(
                "Unreal Variable Assignment".to_string(),
                r"\b[A-Z_][A-Z0-9_]+\s*="
                    .to_string(),
                InjectionType::VariableAssignment,
            )
            .with_confidence(0.7)
            .with_language("C++".to_string())
            .with_description("Potential global/config variable assignment".to_string()),
        );

        // Resource Loading
        self.patterns.push(
            InjectionPattern::new(
                "Unreal Static Load".to_string(),
                r"\b(StaticLoadObject|StaticLoadClass|LoadObject|LoadClass)\s*<"
                    .to_string(),
                InjectionType::ResourceLoad,
            )
            .with_confidence(0.95)
            .with_language("C++".to_string())
            .with_description("Unreal static resource loading".to_string()),
        );

        // Blueprint function library calls
        self.patterns.push(
            InjectionPattern::new(
                "Unreal Blueprint Call".to_string(),
                r"\b(UGameplayStatics|UKismetSystemLibrary|UKismetMathLibrary)\s*::"
                    .to_string(),
                InjectionType::FunctionCall,
            )
            .with_confidence(0.9)
            .with_language("C++".to_string())
            .with_description("Blueprint function library calls".to_string()),
        );

        // Constructor injection
        self.patterns.push(
            InjectionPattern::new(
                "Unreal Constructor".to_string(),
                r"\b(A[[:alpha:]]+\s*::\s*[[:alpha:]]+\(|A[[:alpha:]]+\(\))\s*;"
                    .to_string(),
                InjectionType::Constructor,
            )
            .with_confidence(0.85)
            .with_language("C++".to_string())
            .with_description("Unreal object construction".to_string()),
        );
    }

    /// Add Unity specific patterns
    fn add_unity_patterns(&mut self) {
        // Resource loading
        self.patterns.push(
            InjectionPattern::new(
                "Unity Resource Load".to_string(),
                r"\b(Resources\.Load|Resources\.LoadAsync|AssetDatabase\.LoadAssetAtPath)\s*\("
                    .to_string(),
                InjectionType::ResourceLoad,
            )
            .with_confidence(0.95)
            .with_language("C#".to_string())
            .with_description("Unity resource loading".to_string()),
        );

        // Unity lifecycle methods
        self.patterns.push(
            InjectionPattern::new(
                "Unity Lifecycle Method".to_string(),
                r"\b(void|IEnumerator)\s+(Start|Update|OnEnable|OnDisable|Awake|FixedUpdate|LateUpdate|OnCollision)\s*\("
                    .to_string(),
                InjectionType::EventHook,
            )
            .with_confidence(0.95)
            .with_language("C#".to_string())
            .with_description("Unity lifecycle event hooks".to_string()),
        );

        // Unity component access
        self.patterns.push(
            InjectionPattern::new(
                "Unity Component Get".to_string(),
                r"\b(GetComponent|AddComponent|GetComponentInChildren|GetComponentInParent)\s*<"
                    .to_string(),
                InjectionType::FunctionCall,
            )
            .with_confidence(0.9)
            .with_language("C#".to_string())
            .with_description("Unity component access".to_string()),
        );

        // GameObject instantiation
        self.patterns.push(
            InjectionPattern::new(
                "Unity Instantiate".to_string(),
                r"\b(Instantiate|Object\.Instantiate|GameObject\.Instantiate)\s*\("
                    .to_string(),
                InjectionType::Constructor,
            )
            .with_confidence(0.95)
            .with_language("C#".to_string())
            .with_description("Unity object instantiation".to_string()),
        );

        // SendMessage/Command pattern
        self.patterns.push(
            InjectionPattern::new(
                "Unity SendMessage".to_string(),
                r"\b(SendMessage|BroadcastMessage|SendMessageUpwards)\s*\("
                    .to_string(),
                InjectionType::FunctionCall,
            )
            .with_confidence(0.9)
            .with_language("C#".to_string())
            .with_description("Unity message sending".to_string()),
        );
    }

    /// Add Godot specific patterns
    fn add_godot_patterns(&mut self) {
        // Resource loading
        self.patterns.push(
            InjectionPattern::new(
                "Godot Resource Load".to_string(),
                r"\b(load|preload|ResourceLoader\.load|ResourceLoader\.load_interactive)\s*\("
                    .to_string(),
                InjectionType::ResourceLoad,
            )
            .with_confidence(0.9)
            .with_language("GDScript".to_string())
            .with_description("Godot resource loading".to_string()),
        );

        // Godot lifecycle methods
        self.patterns.push(
            InjectionPattern::new(
                "Godot Lifecycle Method".to_string(),
                r"\bfunc\s+(_ready|_process|_physics_process|_input|_enter_tree|_exit_tree|_on_[[:alpha:]_]+)\s*\("
                    .to_string(),
                InjectionType::EventHook,
            )
            .with_confidence(0.95)
            .with_language("GDScript".to_string())
            .with_description("Godot lifecycle event hooks".to_string()),
        );

        // Node access
        self.patterns.push(
            InjectionPattern::new(
                "Godot Node Access".to_string(),
                r"\b(get_node|find_node|get_child|get_parent|get_tree)\s*\("
                    .to_string(),
                InjectionType::FunctionCall,
            )
            .with_confidence(0.9)
            .with_language("GDScript".to_string())
            .with_description("Godot node access".to_string()),
        );

        // Signal connection
        self.patterns.push(
            InjectionPattern::new(
                "Godot Signal Connect".to_string(),
                r"\bconnect\s*\("
                    .to_string(),
                InjectionType::EventHook,
            )
            .with_confidence(0.85)
            .with_language("GDScript".to_string())
            .with_description("Godot signal connection".to_string()),
        );

        // C# Godot patterns
        self.patterns.push(
            InjectionPattern::new(
                "Godot C# Lifecycle".to_string(),
                r"\b(public\s+override\s+void\s+(_Ready|_Process|_PhysicsProcess|_EnterTree|_ExitTree))\s*\("
                    .to_string(),
                InjectionType::EventHook,
            )
            .with_confidence(0.95)
            .with_language("C#".to_string())
            .with_description("Godot C# lifecycle methods".to_string()),
        );
    }

    /// Add common patterns across engines
    fn add_common_patterns(&mut self) {
        // Dynamic function calls
        self.patterns.push(
            InjectionPattern::new(
                "Dynamic Call".to_string(),
                r"\b(call|invoke|execute|run|eval)\s*\("
                    .to_string(),
                InjectionType::FunctionCall,
            )
            .with_confidence(0.6)
            .with_description("Dynamic function invocation".to_string()),
        );

        // Event registration
        self.patterns.push(
            InjectionPattern::new(
                "Event Registration".to_string(),
                r"\b(add_listener|subscribe|register|on|addEventListener)\s*\("
                    .to_string(),
                InjectionType::EventHook,
            )
            .with_confidence(0.75)
            .with_description("Event listener registration".to_string()),
        );

        // Reflection/API calls
        self.patterns.push(
            InjectionPattern::new(
                "Reflection Call".to_string(),
                r"\b(GetType|GetTypeFromString|FindObject|FindObjectBy)"
                    .to_string(),
                InjectionType::FunctionCall,
            )
            .with_confidence(0.8)
            .with_description("Reflection/API calls".to_string()),
        );

        // File I/O operations
        self.patterns.push(
            InjectionPattern::new(
                "File Operation".to_string(),
                r"\b(File::|std::filesystem|File\.Open|System\.IO\.File|FileAccess\.open)"
                    .to_string(),
                InjectionType::CustomHook,
            )
            .with_confidence(0.85)
            .with_description("File system operations".to_string()),
        );

        // Network operations
        self.patterns.push(
            InjectionPattern::new(
                "Network Operation".to_string(),
                r"\b(HTTP|URL|WebRequest|HttpClient|fetch|axios)\s*"
                    .to_string(),
                InjectionType::CustomHook,
            )
            .with_confidence(0.85)
            .with_description("Network operations".to_string()),
        );
    }

    /// Get all patterns
    pub fn all(&self) -> &[InjectionPattern] {
        &self.patterns
    }

    /// Get patterns for a specific engine
    pub fn for_engine(&self, engine: &EngineType) -> Vec<&InjectionPattern> {
        match engine {
            EngineType::Unreal5_7 => {
                self.patterns
                    .iter()
                    .filter(|p| {
                        p.languages.is_empty()
                            || p.languages.iter().any(|l| l.contains("C++"))
                    })
                    .collect()
            }
            EngineType::Unity2022 => {
                self.patterns
                    .iter()
                    .filter(|p| {
                        p.languages.is_empty()
                            || p.languages.iter().any(|l| l.contains("C#"))
                    })
                    .collect()
            }
            EngineType::Godot4 => {
                self.patterns
                    .iter()
                    .filter(|p| {
                        p.languages.is_empty()
                            || p.languages.iter().any(|l| l.contains("GDScript") || l.contains("C#"))
                    })
                    .collect()
            }
            EngineType::Custom(_) => self.patterns.iter().collect(),
        }
    }

    /// Get pattern by name
    pub fn by_name(&self, name: &str) -> Option<&InjectionPattern> {
        self.patterns.iter().find(|p| p.name == name)
    }

    /// Get patterns by injection type
    pub fn by_type(&self, injection_type: &InjectionType) -> Vec<&InjectionPattern> {
        self.patterns
            .iter()
            .filter(|p| &p.injection_type == injection_type)
            .collect()
    }

    /// Add a custom pattern
    pub fn add_pattern(&mut self, pattern: InjectionPattern) {
        self.patterns.push(pattern);
    }

    /// Remove pattern by name
    pub fn remove_pattern(&mut self, name: &str) -> bool {
        if let Some(pos) = self.patterns.iter().position(|p| p.name == name) {
            self.patterns.remove(pos);
            true
        } else {
            false
        }
    }

    /// Create empty pattern library
    pub fn empty() -> Self {
        Self {
            patterns: Vec::new(),
        }
    }
}

impl Default for PatternLibrary {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pattern_library_creation() {
        let library = PatternLibrary::new();
        assert!(!library.all().is_empty());
    }

    #[test]
    fn test_unreal_patterns() {
        let library = PatternLibrary::new();
        let patterns = library.for_engine(&EngineType::Unreal5_7);
        assert!(!patterns.is_empty());

        let process_call = patterns.iter().find(|p| p.name == "Unreal Process Call");
        assert!(process_call.is_some());
    }

    #[test]
    fn test_unity_patterns() {
        let library = PatternLibrary::new();
        let patterns = library.for_engine(&EngineType::Unity2022);
        assert!(!patterns.is_empty());

        let resource_load = patterns.iter().find(|p| p.name == "Unity Resource Load");
        assert!(resource_load.is_some());
    }

    #[test]
    fn test_godot_patterns() {
        let library = PatternLibrary::new();
        let patterns = library.for_engine(&EngineType::Godot4);
        assert!(!patterns.is_empty());

        let lifecycle = patterns.iter().find(|p| p.name == "Godot Lifecycle Method");
        assert!(lifecycle.is_some());
    }

    #[test]
    fn test_pattern_by_name() {
        let library = PatternLibrary::new();
        let pattern = library.by_name("Unreal Process Call");
        assert!(pattern.is_some());
        assert_eq!(pattern.unwrap().injection_type, InjectionType::FunctionCall);
    }

    #[test]
    fn test_pattern_by_type() {
        let library = PatternLibrary::new();
        let patterns = library.by_type(&InjectionType::ResourceLoad);
        assert!(!patterns.is_empty());
    }

    #[test]
    fn test_add_custom_pattern() {
        let mut library = PatternLibrary::empty();
        let custom = InjectionPattern::new(
            "Custom".to_string(),
            r"test".to_string(),
            InjectionType::CustomHook,
        );
        library.add_pattern(custom);
        assert_eq!(library.all().len(), 1);
    }

    #[test]
    fn test_remove_pattern() {
        let mut library = PatternLibrary::new();
        let initial_count = library.all().len();
        let removed = library.remove_pattern("Unreal Process Call");
        assert!(removed);
        assert_eq!(library.all().len(), initial_count - 1);
    }
}
