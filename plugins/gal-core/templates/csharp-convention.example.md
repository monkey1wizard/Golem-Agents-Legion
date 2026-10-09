<!--
Example Personal Convention File — NOT loaded by GAL Core.

This is a labeled example of a personal (source-2) convention file, adapted from
GAL's former bundled csharp.md. It ships inert: templates/ is read by neither the
convention selector nor the instruction corpus, so it never activates on its own.

To use it: copy this file to `~/.gal/local/conventions/csharp.md` (adjust content
to your own house style) and it becomes an always-on personal convention, injected
into any repo whose Language line matches C#/.NET, the same way source 1 (gal-core)
conventions are injected. See docs/configuration.md for the full personal-conventions guide.
-->

# C# / .NET Conventions

Target: **.NET 10** / **C# 14**. Use file-scoped namespaces, global usings, CRLF on Windows.

---

## Naming (Microsoft Style)

| Category | Convention | Example |
| --- | --- | --- |
| Classes, Records, Structs | `PascalCase` | `OrderValidation` |
| Interfaces | `I` + `PascalCase` | `IMenuStorage` |
| Methods, Properties | `PascalCase` | `ValidateAsync()` |
| Private instance/static fields | `_camelCase` | `_logger`, `_jsonOptions` |
| Constants (`const`) | `PascalCase` | `MaxRetries` |
| Local variables, parameters | `camelCase` | `orderCount` |
| Enums | `PascalCase` | `OrderState.Pending` |
| Type parameters | `T` prefix | `TResult` |
| Domain acronyms | Full caps | `LLM`, `ASR`, `API` |

---

## Modern C# Syntax

Always prefer newest syntax. Selection criteria: reads closer to English, then more concise.

| Feature (C# ver) | Prefer | Avoid |
| --- | --- | --- |
| Null-conditional assignment (14) | `customer?.Order = GetOrder();` | `if (customer is not null) ...` |
| `field` keyword (14) | `set { field = value.Trim(); }` | Manual `_name` backing field |
| Extension members (14) | `extension(Order) { ... }` | `static class OrderExtensions` |
| Collection expressions (12) | `[1, 2, 3]` | `new int[] { 1, 2, 3 }` |
| Primary constructors (12) | `class Svc(IDep dep)` | Manual ctor + `_dep` field |
| Raw string literals (11) | `"""{ "key": "val" }"""` | `"{ \"key\": \"val\" }"` |
| Required members (11) | `required string Name` | Ctor parameter for mandatory props |
| Pattern matching (9+) | `is null`, `is not null` | `== null`, `!= null` |
| Target-typed `new` (9) | `List<Order> orders = new();` | `var orders = new List<Order>();` when type obvious |
| Switch expressions (8) | `x switch { 1 => "a", _ => "b" }` | `switch` statement |

### Syntax Decision Tree

```text
Store injected dependency?         → Primary constructor
Backing field with custom logic?   → field keyword (C# 14)
Observable property in ViewModel?  → [ObservableProperty] partial property
Initialize a collection?           → Collection expression
Check for null?                    → Pattern matching (is null / is not null)
Branch on a value?                 → Switch expression
```

---

## Primary Constructors

Always use. No manual backing fields for injected services.

```csharp
// ✅
public class OrderService(IRepository repository, ILogger<OrderService> logger)
{
    public async Task ProcessAsync() => await repository.SaveAsync();
}
```

## ViewModels (CommunityToolkit.Mvvm)

Inherit `ObservableObject`. Use `[ObservableProperty]` with partial property syntax (8.4+).

```csharp
public partial class OrderViewModel(ILLMService llmService) : ObservableObject
{
    [ObservableProperty]
    public partial bool IsRecording { get; set; }

    [RelayCommand]
    private async Task ToggleRecordingAsync() => await llmService.StartAsync();
}
```

## Async/Await

- Always `async/await`. **Never** `.Result` or `.Wait()` (causes deadlocks).
- Use Constructor Injection. Never service locator pattern.

## Documentation Comments

**Self-documenting names over comments.** Only write `/// <summary>` when the name alone cannot convey non-obvious behavior, side effects, or constraints. Use `//` only for "why", never for "what".

---

## Clean Architecture

### Layer Dependencies

```text
[ Presentation ]    [ Infrastructure ]
         │                  │
         └────────┬─────────┘
                  ▼
           [ Application ]
                  │
                  ▼
             [ Domain ]
```

- **Domain**: Zero dependencies (entities, value objects, domain services)
- **Application**: Depends only on Domain (use cases, interfaces, DTOs)
- **Infrastructure**: Implements Application interfaces (DB, API, file system)
- **Presentation**: Depends on Application (ViewModels, UI)

### Placement Decision Tree

```text
New class → What does it do?
    ├─ Pure business rule, entity, value object?        → Domain
    ├─ Orchestrates use cases, defines interfaces?      → Application
    ├─ Talks to external systems (DB, API, LLM)?       → Infrastructure
    └─ Drives UI (ViewModel, page logic)?               → Presentation
```

### Key Rules

- Dependencies point inward. Inner layers must not know about outer layers.
- **Never duplicate folder names across layers** (use `UseCases/`, `DomainServices/`, `ExternalServices/` instead of `Services/` everywhere).
- Register DI in composition root (entry point), not in inner layers.

### Recommended Folders

| Layer | Folders |
| --- | --- |
| Domain | Entities, Models, Common, Constants, Enums |
| Application | Interfaces, Services, Stubs |
| Infrastructure | Adapters, LLMProviders, Storage, ASR |
| Presentation | ViewModels, Resources |

---

## Blazor

### Component Structure

- `PascalCase` for component files (`OrderList.razor`)
- Simple components: `@code { }` in `.razor`. Complex: partial class (`.razor.cs`)
- `[Parameter]` must be `public`. Use `[EditorRequired]` for mandatory. Use `[SupplyParameterFromQuery]` for URL params.
- Global `@using`/`@inject` in `_Imports.razor`. Don't duplicate in individual files.

### Events and Lifecycle

- Use `@onclick="MethodName"` (delegate) over lambda when no args needed
- Always use `Async` lifecycle methods (`OnInitializedAsync`)
- Implement `IDisposable` for event subscription cleanup

### Styling

- **MudBlazor first**: Use MudBlazor components over raw HTML
- Bootstrap utility classes for layout/spacing
- Custom CSS via CSS Isolation (`.razor.css`) with `kebab-case` classes
- Avoid inline styles

### Error Handling

- Wrap major UI sections in `<ErrorBoundary>`
- Follow Result pattern for logic errors

---

## Godot Runtime Rules

When code is loaded by the Godot runtime, compatibility takes priority over the newest language features in this document.

### Runtime Compatibility

| Code kind | Target | Guidance |
| --- | --- | --- |
| Godot node scripts and editor plugins | Godot-supported runtime for the project (currently .NET 8 / C# 12 baseline) | Do not use language features newer than the runtime Godot can compile and load |
| MCP servers, CLI tooling, and external automation | .NET 10 / C# 14 | Follow the rest of this document normally |

If a repo mixes Godot game code and external tooling, treat them as separate compatibility zones.

### Node Script Rules

- Godot node types must be `public partial class` and import `using Godot;`
- Do not use constructors or primary constructors on Godot node scripts; Godot instantiates these types itself
- Initialize scene references in `_Ready()` instead of constructor logic
- Keep Inspector-configurable data in `[Export]` members
- Use explicit `override` methods for lifecycle hooks

### Godot Attribute Mapping

| GDScript concept | C# pattern | Notes |
| --- | --- | --- |
| `@export` | `[Export]` | Add `PropertyHint` when Inspector constraints matter |
| `signal foo` | `[Signal] public delegate void FooEventHandler(...);` | Generated event name drops the `EventHandler` suffix |
| `@onready` | Assign in `_Ready()` | Use `GetNode<T>()` or cached node references |
| `await some_signal` | `await ToSignal(node, Node.SignalName.X)` | Prefer generated C# events for normal subscriptions |

### Lifecycle Patterns

- `_Ready()` for scene wiring and node lookup
- `_Process(double delta)` for frame-based logic
- `_PhysicsProcess(double delta)` for physics movement and collision work
- `_EnterTree()` and `_ExitTree()` for registration and cleanup that must track tree membership

### API and Collection Guidance

- Use PascalCase Godot APIs in C#: `GetNode`, `QueueFree`, `MoveAndSlide`, `Input.IsActionPressed`
- Use `GD.Print`, `GD.PushWarning`, and `GD.PushError` for Godot-facing diagnostics
- Prefer `List<T>` and `Dictionary<TKey, TValue>` for internal app logic
- Use `Godot.Collections.Array<T>`, `Godot.Collections.Dictionary`, and `Variant` when crossing the Godot API boundary
- Prefer `StringName`-based signal and method identifiers when the API exposes them

### Example Node Script

```csharp
using Godot;

public partial class PlayerController : CharacterBody2D
{
    [Export]
    public float Speed { get; set; } = 220.0f;

    private Sprite2D _sprite = null!;

    public override void _Ready()
    {
        _sprite = GetNode<Sprite2D>("Sprite2D");
    }

    public override void _PhysicsProcess(double delta)
    {
        var direction = Input.GetVector("move_left", "move_right", "move_up", "move_down");
        Velocity = direction * Speed;
        MoveAndSlide();

        if (direction.X != 0)
        {
            _sprite.FlipH = direction.X < 0;
        }
    }
}
```

---

## .NET CLI Tooling

These are optional lanes. Verify the `dotnet` CLI is available before using them. When unavailable, fall back to IDE-based output or direct file inspection.

### Filtered Output Modes

Prefer filtered output over raw verbosity to reduce noise.

| Task | Preferred command | Notes |
| --- | --- | --- |
| Build (failure summary) | `dotnet build --verbosity minimal` | Emits errors and warnings only |
| Test (failures only) | `dotnet test --logger "console;verbosity=minimal"` | Emits failing test names + messages; suppresses passing output |
| Test (specific filter) | `dotnet test --filter "FullyQualifiedName~MyTest"` | Narrow to the relevant test class before running |
| Lint / format check | `dotnet format --verify-no-changes` | Exit code 1 if changes needed; no diff noise on pass |

### Evidence Collection Rules

- Capture only: first build error + file:line reference, or failing test names + assertion messages.
- Do not pipe full build logs into context. Store them on disk; retrieve specific lines when diagnosis requires them.
- On a clean pass, emit one confirmation line only (`Build succeeded` / `X tests passed`).

**Preflight**: confirm `dotnet --version` succeeds before dispatching any of the above. If the CLI is absent, note this in the task log and proceed with fallback inspection.
