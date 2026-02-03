use std::collections::HashMap;

/// Definition of a signal or method argument.
pub struct ArgumentDefinition {
    name: String,
    typ: String,
    annotations: HashMap<String, String>,
}

impl ArgumentDefinition {
    /// Create a new ArgumentDefinition.
    ///
    /// # Arguments
    ///
    /// * `name` - The name of the argument.
    /// * `typ` - The D-Bus type of the argument.
    pub fn new(name: String, typ: String) -> Self {
        Self { name, typ, annotations: HashMap::new() }
    }

    /// Add an annotation to the argument.
    ///
    /// # Arguments
    ///
    /// * `key` - The key of the annotation.
    /// * `value` - The value of the annotation.
    pub fn add_annotation(&mut self, key: String, value: String) {
        self.annotations.insert(key, value);
    }

    /// Get the name of the argument.
    pub fn get_name(&self) -> &str {
        &self.name
    }

    /// Get the type of the argument.
    pub fn get_type(&self) -> &str {
        &self.typ
    }

    /// Get the annotations of the argument.
    pub fn get_annotations(&self) -> &HashMap<String, String> {
        &self.annotations
    }
}

/// Definition of a signal.
pub struct SignalDefinition {
    name: String,
    args: Vec<ArgumentDefinition>,
    annotations: HashMap<String, String>,
}

impl SignalDefinition {
    /// Create a new SignalDefinition.
    ///
    /// # Arguments
    ///
    /// * `name` - The name of the signal.
    pub fn new(name: String) -> Self {
        Self { name, args: Vec::new(), annotations: HashMap::new() }
    }

    /// Add an argument to the signal.
    ///
    /// # Arguments
    ///
    /// * `arg` - The ArgumentDefinition to add.
    pub fn add_arg(&mut self, arg: ArgumentDefinition) {
        self.args.push(arg);
    }

    /// Add an annotation to the argument.
    ///
    /// # Arguments
    ///
    /// * `key` - The key of the annotation.
    /// * `value` - The value of the annotation.
    pub fn add_annotation(&mut self, key: String, value: String) {
        self.annotations.insert(key, value);
    }

    /// Get an iterator over the arguments of the signal.
    pub fn iter_args(&self) -> impl Iterator<Item = &ArgumentDefinition> {
        self.args.iter()
    }

    /// Get the name of the signal.
    pub fn get_name(&self) -> &str {
        &self.name
    }

    /// Get a the annotations of the signal.
    pub fn get_annotations(&self) -> &HashMap<String, String> {
        &self.annotations
    }
}

/// Definition of a method.
pub struct MethodDefinition {
    name: String,
    input_args: Vec<ArgumentDefinition>,
    output_args: Vec<ArgumentDefinition>,
    annotations: HashMap<String, String>,
}

impl MethodDefinition {
    /// Create a new MethodDefinition.
    ///
    /// # Arguments
    ///
    /// * `name` - The name of the method.
    pub fn new(name: String) -> Self {
        Self { name, input_args: Vec::new(), output_args: Vec::new(), annotations: HashMap::new() }
    }

    /// Add an input argument to the method.
    ///
    /// # Arguments
    ///
    /// * `arg` - The ArgumentDefinition to add as an input argument.
    pub fn add_input_arg(&mut self, arg: ArgumentDefinition) {
        self.input_args.push(arg);
    }

    /// Add an output argument to the method.
    ///
    /// # Arguments
    ///
    /// * `arg` - The ArgumentDefinition to add as an output argument.
    pub fn add_output_arg(&mut self, arg: ArgumentDefinition) {
        self.output_args.push(arg);
    }

    /// Add an annotation to the argument.
    ///
    /// # Arguments
    ///
    /// * `key` - The key of the annotation.
    /// * `value` - The value of the annotation.
    pub fn add_annotation(&mut self, key: String, value: String) {
        self.annotations.insert(key, value);
    }

    /// Get the name of the method.
    pub fn get_name(&self) -> &str {
        &self.name
    }

    /// Get an iterator over the input arguments of the method.
    pub fn iter_input_args(&self) -> impl Iterator<Item = &ArgumentDefinition> {
        self.input_args.iter()
    }

    /// Get an iterator over the output arguments of the method.
    pub fn iter_output_args(&self) -> impl Iterator<Item = &ArgumentDefinition> {
        self.output_args.iter()
    }

    /// Get the annotations of the method.
    pub fn get_annotations(&self) -> &HashMap<String, String> {
        &self.annotations
    }
}

/// Definition of a property.
pub struct PropertyDefinition {
    name: String,
    typ: String,
    access: String,
    annotations: HashMap<String, String>,
}

impl PropertyDefinition {
    /// Create a new PropertyDefinition.
    ///
    /// # Arguments
    ///
    /// * `name` - The name of the property.
    /// * `typ` - The D-Bus type of the property.
    /// * `access` - The access type of the property (e.g., "read", "write", "readwrite").
    pub fn new(name: String, typ: String, access: String) -> Self {
        Self { name, typ, access, annotations: HashMap::new() }
    }

    /// Add an annotation to the property.
    ///
    /// # Arguments
    ///
    /// * `key` - The key of the annotation.
    /// * `value` - The value of the annotation.
    pub fn add_annotation(&mut self, key: String, value: String) {
        self.annotations.insert(key, value);
    }

    /// Get the name of the property.
    pub fn get_name(&self) -> &str {
        &self.name
    }

    /// Get the type of the property.
    pub fn get_type(&self) -> &str {
        &self.typ
    }

    /// Get the access type of the property.
    pub fn get_access(&self) -> &str {
        &self.access
    }

    /// Get the annotations of the property.
    pub fn get_annotations(&self) -> &HashMap<String, String> {
        &self.annotations
    }
}

/// Definition of an interface.
pub struct InterfaceDefinition {
    name: String,
    signals: Vec<SignalDefinition>,
    methods: Vec<MethodDefinition>,
    properties: Vec<PropertyDefinition>,
    annotations: HashMap<String, String>,
}

impl InterfaceDefinition {
    /// Create a new InterfaceDefinition.
    ///
    /// # Arguments
    ///
    /// * `name` - The name of the interface.
    pub fn new(name: String) -> Self {
        Self { name, signals: Vec::new(), methods: Vec::new(), properties: Vec::new(), annotations: HashMap::new() }
    }

    /// Add a signal to the interface.
    ///
    /// # Arguments
    ///
    /// * `signal` - The SignalDefinition to add.
    pub fn add_signal(&mut self, signal: SignalDefinition) {
        self.signals.push(signal);
    }

    /// Add a method to the interface.
    ///
    /// # Arguments
    ///
    /// * `method` - The MethodDefinition to add.
    pub fn add_method(&mut self, method: MethodDefinition) {
        self.methods.push(method);
    }

    /// Add a property to the interface.
    ///
    /// # Arguments
    ///
    /// * `property` - The PropertyDefinition to add.
    pub fn add_property(&mut self, property: PropertyDefinition) {
        self.properties.push(property);
    }

    /// Add an annotation to the argument.
    ///
    /// # Arguments
    ///
    /// * `key` - The key of the annotation.
    /// * `value` - The value of the annotation.
    pub fn add_annotation(&mut self, key: String, value: String) {
        self.annotations.insert(key, value);
    }

    /// Get the name of the interface.
    pub fn get_name(&self) -> &str {
        &self.name
    }

    /// Get an iterator over the signals of the interface.
    pub fn iter_signals(&self) -> impl Iterator<Item = &SignalDefinition> {
        self.signals.iter()
    }

    /// Get an iterator over the methods of the interface.
    pub fn iter_methods(&self) -> impl Iterator<Item = &MethodDefinition> {
        self.methods.iter()
    }

    /// Get an iterator over the properties of the interface.
    pub fn iter_properties(&self) -> impl Iterator<Item = &PropertyDefinition> {
        self.properties.iter()
    }

    /// Get the annotations of the interface.
    pub fn get_annotations(&self) -> &HashMap<String, String> {
        &self.annotations
    }
}
