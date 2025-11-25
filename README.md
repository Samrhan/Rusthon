# Compilateur Python vers LLVM en Rust : guide complet pour un POC

Un compilateur Python en Rust avec LLVM est réalisable en **3 semaines** avec l'approche incrémentale. **Inkwell** (wrapper LLVM) combiné à **rustpython-parser** constitue le stack technologique optimal pour ce POC. Les projets similaires comme Numba et Codon démontrent qu'un sous-ensemble Python compilé peut atteindre des performances **10-100x supérieures** à CPython, à condition de restreindre le typage dynamique.

---

## Bindings LLVM : Inkwell domine pour un POC

Pour un compilateur Python en Rust, **Inkwell est la recommandation claire**. Ce wrapper safe autour de llvm-sys offre une API idiomatique Rust avec typage fort au niveau des types LLVM, interceptant les erreurs à la compilation plutôt qu'au runtime.

### Inkwell : avantages et mise en place

Inkwell (858K+ téléchargements, version 0.7.0) supporte LLVM 8-21 via feature flags. Son architecture repose sur trois concepts : `Context` (gère les ressources LLVM), `Module` (unité de compilation), et `Builder` (construit les instructions IR).

```toml
# Cargo.toml
[dependencies]
inkwell = { version = "0.7.0", features = ["llvm18-0"] }
```

```rust
use inkwell::context::Context;
use inkwell::OptimizationLevel;

type SumFunc = unsafe extern "C" fn(u64, u64) -> u64;

fn main() {
    let context = Context::create();
    let module = context.create_module("calculateur");
    let builder = context.create_builder();
    
    // Définir la fonction: i64 add(i64 a, i64 b)
    let i64_type = context.i64_type();
    let fn_type = i64_type.fn_type(&[i64_type.into(), i64_type.into()], false);
    let function = module.add_function("add", fn_type, None);
    
    // Créer le bloc d'entrée
    let entry = context.append_basic_block(function, "entry");
    builder.position_at_end(entry);
    
    // Récupérer les paramètres et construire l'addition
    let x = function.get_nth_param(0).unwrap().into_int_value();
    let y = function.get_nth_param(1).unwrap().into_int_value();
    let sum = builder.build_int_add(x, y, "sum").unwrap();
    builder.build_return(Some(&sum)).unwrap();
    
    // Compilation JIT et exécution
    let engine = module.create_jit_execution_engine(OptimizationLevel::None).unwrap();
    unsafe {
        let add_fn: inkwell::execution_engine::JitFunction<SumFunc> = 
            engine.get_function("add").unwrap();
        println!("5 + 3 = {}", add_fn.call(5, 3)); // Output: 8
    }
}
```

### Cranelift : alternative pure Rust

Cranelift (du Bytecode Alliance) compile **10x plus vite** que LLVM mais produit du code ~14% moins performant. Écrit entièrement en Rust (~200K lignes vs 20M+ pour LLVM), il simplifie radicalement la configuration : pas d'installation LLVM requise.

| Critère | Inkwell/LLVM | Cranelift |
|---------|--------------|-----------|
| Vitesse de compilation | Plus lent | **10x plus rapide** |
| Qualité du code | **Optimal** | -14% |
| Setup | Requiert LLVM | Pure Rust |
| Cas d'usage | Production AOT | JIT, debug |

**Verdict** : Inkwell pour le POC. Les tutoriels Kaleidoscope et le livre "Create Your Own Language with Rust" (createlang.rs) utilisent Inkwell, facilitant l'apprentissage.

---

## Parser Python : rustpython-parser recommandé

Pour parser le sous-ensemble Python ciblé, **rustpython-parser** (50K+ téléchargements/mois, utilisé par Ruff) offre la meilleure balance maturité/API.

### Intégration de rustpython-parser

```toml
[dependencies]
rustpython-parser = "0.4"
```

```rust
use rustpython_parser::{Parse, ast};

fn main() {
    let source = r#"
def add(a, b):
    return a + b

x = add(1, 2)
print(x)
"#;

    // Parser le code en AST
    let statements = ast::Suite::parse(source, "<input>").unwrap();
    
    for stmt in &statements {
        println!("{:#?}", stmt);
    }
}
```

### Conversion AST vers IR personnalisée

Le pattern clé consiste à définir une IR intermédiaire simplifiée, puis à "lower" l'AST Python vers cette IR :

```rust
// Définition de l'IR simplifiée pour le sous-ensemble
#[derive(Debug, Clone)]
enum IRExpr {
    Constant(i64),
    Float(f64),
    Variable(String),
    BinaryOp { op: BinOp, left: Box<IRExpr>, right: Box<IRExpr> },
    Call { func: String, args: Vec<IRExpr> },
}

#[derive(Debug, Clone)]
enum IRStmt {
    Assign { target: String, value: IRExpr },
    Print(IRExpr),
    FunctionDef { name: String, params: Vec<String>, body: Vec<IRStmt> },
    Return(IRExpr),
}

// Lowering de l'AST Python vers l'IR
use rustpython_parser::ast::{self, Stmt, Expr, Operator};

fn lower_expr(expr: &Expr) -> Result<IRExpr, String> {
    match expr {
        Expr::Constant(c) => match &c.value {
            ast::Constant::Int(n) => Ok(IRExpr::Constant(n.as_i64().unwrap())),
            ast::Constant::Float(f) => Ok(IRExpr::Float(*f)),
            _ => Err("Type de constante non supporté".into()),
        },
        Expr::Name(name) => Ok(IRExpr::Variable(name.id.to_string())),
        Expr::BinOp(binop) => {
            let op = match binop.op {
                Operator::Add => BinOp::Add,
                Operator::Sub => BinOp::Sub,
                Operator::Mult => BinOp::Mul,
                Operator::Div => BinOp::Div,
                _ => return Err("Opérateur non supporté".into()),
            };
            Ok(IRExpr::BinaryOp {
                op,
                left: Box::new(lower_expr(&binop.left)?),
                right: Box::new(lower_expr(&binop.right)?),
            })
        }
        _ => Err(format!("Expression non supportée: {:?}", expr)),
    }
}
```

### Faut-il écrire son propre parser ?

Pour un vrai sous-ensemble avec syntaxe modifiée, **pest** (PEG parser generator) est excellent. Pour un sous-ensemble Python standard, rustpython-parser économise des semaines de travail tout en garantissant la compatibilité syntaxique.

---

## Projets similaires : leçons architecturales

L'analyse de Numba, Codon, Nuitka et Mojo révèle des patterns essentiels pour gérer le typage dynamique de Python dans un contexte compilé.

### Numba : le modèle JIT par inférence de types

Numba compile un sous-ensemble numérique de Python via LLVM avec des speedups de **100x+**. Son approche : inférer les types au premier appel, puis spécialiser le code généré.

```
Pipeline Numba:
Python Bytecode → Numba IR (SSA) → Inférence de types → LLVM IR → Code natif
```

**Leçon clé** : Le mode "nopython" (compilation complète sans fallback Python) donne les meilleures performances. Définir explicitement ce qui est supporté et rejeter le reste.

### Codon : compilateur AOT haute performance

Codon (MIT) produit des performances comparables au C/C++ pour un sous-ensemble Python via LLVM. Son IR intermédiaire est plus haut niveau que LLVM IR, permettant des optimisations spécifiques (reconnaissance de patterns comme `for x in range(n)`).

**Leçon clé** : Une IR intermédiaire entre l'AST et LLVM IR facilite les optimisations domain-specific.

### Nuitka vs Mojo : deux philosophies

| Aspect | Nuitka | Mojo |
|--------|--------|------|
| Compatibilité | 100% Python | Superset avec nouvelles features |
| Speedup | 1-4x | 10-35000x |
| Approche | Transpilation → C → CPython embedded | Static typing + MLIR/LLVM |
| Trade-off | Compatibilité max | Performance max |

**Leçon pour le POC** : Cibler un sous-ensemble restreint comme Numba/Codon plutôt que la compatibilité totale de Nuitka.

---

## Architecture recommandée pour le compilateur

Le pipeline classique s'adapte bien à un compilateur Python en Rust :

```
Source Python → Lexer → Parser → AST → IR (optionnel) → LLVM IR → Code natif
              └────── rustpython-parser ──────┘   └── Inkwell ──┘
```

### Gestion du typage dynamique

Trois stratégies possibles, classées par complexité croissante :

**Stratégie 1 : Types monomorphes (recommandée pour le POC)**
- Tous les nombres sont `f64` (comme Kaleidoscope)
- Pas de polymorphisme, erreur si types incompatibles
- Simple à implémenter, performances optimales

**Stratégie 2 : Tagged unions (boxed values)**
```rust
// Représentation en Rust
enum DynamicValue {
    Integer(i64),
    Float(f64),
    Boolean(bool),
}

// Représentation LLVM
// %DynamicValue = type { i8, [8 x i8] }  // tag + payload
```

Chaque opération vérifie les tags au runtime et dispatch vers le code approprié.

**Stratégie 3 : Inférence de types à la compilation**
Comme Numba : analyser le flux de données pour déduire les types concrets, spécialiser le code, fallback si échec.

### Code generation LLVM pour arithmétique

```rust
use inkwell::context::Context;
use inkwell::builder::Builder;
use inkwell::FloatPredicate;

fn compile_binary_op(
    context: &Context,
    builder: &Builder,
    op: BinOp,
    lhs: inkwell::values::FloatValue,
    rhs: inkwell::values::FloatValue,
) -> inkwell::values::FloatValue {
    match op {
        BinOp::Add => builder.build_float_add(lhs, rhs, "addtmp").unwrap(),
        BinOp::Sub => builder.build_float_sub(lhs, rhs, "subtmp").unwrap(),
        BinOp::Mul => builder.build_float_mul(lhs, rhs, "multmp").unwrap(),
        BinOp::Div => builder.build_float_div(lhs, rhs, "divtmp").unwrap(),
    }
}

// Comparaisons
fn compile_comparison(builder: &Builder, lhs: FloatValue, rhs: FloatValue) -> IntValue {
    builder.build_float_compare(FloatPredicate::OLT, lhs, rhs, "cmptmp").unwrap()
}
```

### Génération de fonctions complètes

```rust
fn compile_function(
    context: &Context,
    module: &Module,
    builder: &Builder,
    name: &str,
    params: &[String],
    body: &[IRStmt],
) -> Result<FunctionValue, String> {
    let f64_type = context.f64_type();
    
    // Créer la signature : tous les params sont f64
    let param_types: Vec<_> = params.iter().map(|_| f64_type.into()).collect();
    let fn_type = f64_type.fn_type(&param_types, false);
    let function = module.add_function(name, fn_type, None);
    
    // Nommer les paramètres
    for (i, arg) in function.get_param_iter().enumerate() {
        arg.into_float_value().set_name(&params[i]);
    }
    
    // Créer le bloc d'entrée
    let entry = context.append_basic_block(function, "entry");
    builder.position_at_end(entry);
    
    // Table des symboles pour les variables locales
    let mut variables: HashMap<String, FloatValue> = HashMap::new();
    for (i, arg) in function.get_param_iter().enumerate() {
        variables.insert(params[i].clone(), arg.into_float_value());
    }
    
    // Compiler le corps
    // ... compiler chaque statement
    
    Ok(function)
}
```

### I/O : liaison avec la libc

```rust
use inkwell::module::Linkage;
use inkwell::AddressSpace;

fn add_printf(context: &Context, module: &Module) {
    let i32_type = context.i32_type();
    let i8_ptr = context.i8_type().ptr_type(AddressSpace::default());
    
    // int printf(const char* format, ...)
    let printf_type = i32_type.fn_type(&[i8_ptr.into()], true); // varargs
    module.add_function("printf", printf_type, Some(Linkage::External));
}

fn build_print(builder: &Builder, module: &Module, value: FloatValue) {
    let printf = module.get_function("printf").unwrap();
    let format = builder.build_global_string_ptr("%f\n", "fmt").unwrap();
    
    builder.build_call(
        printf,
        &[format.as_pointer_value().into(), value.into()],
        "printf_call"
    ).unwrap();
}
```

---

## Tutoriels et ressources essentielles

### Ressources primaires

- **Kaleidoscope en Rust avec Inkwell** : implémentation complète dans `inkwell/examples/kaleidoscope/`
- **"Create Your Own Programming Language with Rust"** : createlang.rs - le guide le plus complet
- **LLVM Tutorial adapté** : github.com/acolite-d/llvm-tutorial-in-rust-using-inkwell
- **Iron Kaleidoscope** : github.com/jauhien/iron-kaleidoscope

### Documentation technique

- **Inkwell docs** : thedan64.github.io/inkwell/
- **LLVM Language Reference** : llvm.org/docs/LangRef.html (indispensable pour comprendre l'IR)
- **AOSA Book - LLVM** : aosabook.org/en/v1/llvm.html (architecture de LLVM expliquée)

### L'approche Ghuloum

Le paper "An Incremental Approach to Compiler Construction" de Abdulaziz Ghuloum est fondamental. **Principe clé** : ne pas construire lexer → parser → codegen séquentiellement, mais plutôt en "tranches verticales" end-to-end.

---

## Plan d'action POC : 3 semaines

### Semaine 1 : fondations et premier compilateur fonctionnel

**Jours 1-2 : Setup**
```bash
# Installation LLVM (macOS)
brew install llvm@18
export LLVM_SYS_180_PREFIX=$(brew --prefix llvm@18)

# Création du projet
cargo new python-compiler && cd python-compiler
```

**Jours 3-4 : print(42) fonctionne**
- Parser minimal (rustpython-parser)
- Codegen pour les entiers littéraux
- Liaison avec printf
- **Test** : `./compiler "print(42)"` → affiche "42"

**Jours 5-7 : Arithmétique complète**
- Opérateurs `+ - * / %`
- Précédence des opérateurs
- **Test** : `print(1 + 2 * 3)` → "7"

### Semaine 2 : variables et fonctions

**Jours 8-9 : Variables**
```python
# Objectif
x = 5
y = x + 3
print(y)  # → 8
```
- Table des symboles
- Allocation stack (alloca LLVM)

**Jours 10-12 : Fonctions**
```python
# Objectif
def add(a, b):
    return a + b

print(add(1, 2))  # → 3
```
- Déclaration de fonctions
- Passage de paramètres
- Return

**Jours 13-14 : Types flottants**
- Support des floats (`3.14`)
- Arithmétique mixte int/float

### Semaine 3 : I/O et finalisation

**Jours 15-16 : Input**
```python
x = input()
print(x)
```
- FFI vers getchar/scanf
- Conversion string → nombre (optionnel)

**Jours 17-18 : Gestion d'erreurs**
- Messages d'erreur avec ligne/colonne
- Erreurs de type claires

**Jours 19-21 : Tests et documentation**
- 50+ tests couvrant toutes les features
- Snapshot tests pour l'IR généré (crate `insta`)
- README avec exemples
- Liste des limitations documentée

### Structure du projet recommandée

```
python-compiler/
├── Cargo.toml
├── src/
│   ├── main.rs           # CLI
│   ├── lexer.rs          # Tokenization (ou utiliser rustpython-parser)
│   ├── parser.rs         # AST depuis rustpython-parser
│   ├── ast.rs            # IR simplifiée
│   ├── lowering.rs       # AST Python → IR
│   ├── codegen.rs        # IR → LLVM IR (Inkwell)
│   └── compiler.rs       # Orchestration
└── tests/
    ├── arithmetic.rs
    ├── variables.rs
    └── functions.rs
```

### Cargo.toml complet

```toml
[package]
name = "python-compiler"
version = "0.1.0"
edition = "2021"

[dependencies]
inkwell = { version = "0.7.0", features = ["llvm18-0"] }
rustpython-parser = "0.4"
thiserror = "1.0"
ariadne = "0.4"     # Messages d'erreur élégants

[dev-dependencies]
insta = "1.34"      # Snapshot testing
```

---

## Milestones et critères de succès

| Milestone | Critère de validation |
|-----------|----------------------|
| M1 : Setup | `cargo build` réussit avec LLVM linké |
| M2 : Hello World | `print(42)` produit un exécutable fonctionnel |
| M3 : Arithmétique | `print((1+2)*3-4/2)` → résultat correct |
| M4 : Variables | `x=5; y=x+1; print(y)` → "6" |
| M5 : Fonctions | `def f(x): return x*2; print(f(21))` → "42" |
| M6 : Floats | `print(3.14 + 1.0)` → "4.14" |
| M7 : I/O | `x=input(); print(x)` → fonctionne |
| M8 : POC complet | 50+ tests passent, documentation prête |

### Pièges à éviter

- **Scope creep** : Définir explicitement ce qui est hors-scope (classes, modules, exceptions, list comprehensions)
- **Over-engineering l'AST** : Commencer avec 5-6 types de nœuds, pas 50
- **`.unwrap()` partout** : Utiliser `Result<T, CompileError>` dès le départ avec `thiserror`
- **Pas de tests** : Écrire le test AVANT la feature (TDD)
- **Construire séquentiellement** : Toujours avoir un compilateur fonctionnel, même limité

Ce POC pose les fondations pour l'objectif final : un framework de tests Python compilé en code natif pour une exécution rapide. Une fois le sous-ensemble arithmétique et fonctions stabilisé, l'extension vers des constructs de test (`assert`, comparaisons, structures de données) devient incrémentale.
