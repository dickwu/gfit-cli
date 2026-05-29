//! The command registry: every workout-app API endpoint mapped to a dot-named
//! CLI command. This is the single source of truth — `main.rs` drives entirely
//! off this table for dispatch, `-h` docs, body building, and auth.

use serde_json::{json, Value};

#[derive(Clone, Copy, PartialEq)]
pub enum PType {
    Str,
    Num,
    Bool,
    Json,
}

impl PType {
    pub fn label(&self) -> &'static str {
        match self {
            PType::Str => "string",
            PType::Num => "number",
            PType::Bool => "bool",
            PType::Json => "json",
        }
    }

    pub fn coerce(&self, raw: &str) -> Result<Value, String> {
        match self {
            PType::Str => Ok(Value::String(raw.to_string())),
            PType::Num => {
                if let Ok(i) = raw.parse::<i64>() {
                    Ok(json!(i))
                } else if let Ok(f) = raw.parse::<f64>() {
                    Ok(json!(f))
                } else {
                    Err(format!("expected a number, got '{raw}'"))
                }
            }
            PType::Bool => match raw {
                "true" | "1" | "yes" | "y" => Ok(Value::Bool(true)),
                "false" | "0" | "no" | "n" | "" => Ok(Value::Bool(false)),
                _ => Err(format!("expected true/false, got '{raw}'")),
            },
            PType::Json => serde_json::from_str(raw).map_err(|e| format!("invalid JSON: {e}")),
        }
    }
}

/// Loose coercion for undocumented passthrough flags: bool, int, float, else string.
pub fn coerce_loose(raw: &str) -> Value {
    let trimmed = raw.trim_start();
    if trimmed.starts_with('{') || trimmed.starts_with('[') {
        if let Ok(v) = serde_json::from_str::<Value>(raw) {
            return v;
        }
    }
    match raw {
        "true" => return Value::Bool(true),
        "false" => return Value::Bool(false),
        _ => {}
    }
    if let Ok(i) = raw.parse::<i64>() {
        return json!(i);
    }
    if let Ok(f) = raw.parse::<f64>() {
        // Guard against leading-zero / "+1" style strings being mangled.
        if raw == f.to_string() || raw.trim_start_matches('-').starts_with(|c: char| c.is_ascii_digit()) {
            return json!(f);
        }
    }
    Value::String(raw.to_string())
}

pub struct Param {
    pub name: String,
    pub ty: PType,
    pub required: bool,
    /// Constant or default value applied when the flag is absent.
    pub fixed: Option<String>,
    pub desc: String,
}

#[derive(PartialEq)]
pub enum Method {
    Json,
    Multipart,
}

#[derive(PartialEq)]
pub enum Special {
    None,
    Login,
    Logout,
    Status,
    ClientFilter,
    Update,
}

pub struct Command {
    pub name: String,
    pub aliases: Vec<String>,
    pub path: String,
    pub method: Method,
    pub auth: bool,
    pub params: Vec<Param>,
    pub desc: String,
    pub special: Special,
}

impl Command {
    pub fn matches(&self, name: &str) -> bool {
        self.name == name || self.aliases.iter().any(|a| a == name)
    }

    pub fn group(&self) -> &str {
        self.name.split('.').next().unwrap_or(&self.name)
    }

    fn alias(mut self, a: &str) -> Self {
        self.aliases.push(a.to_string());
        self
    }
    fn multipart(mut self) -> Self {
        self.method = Method::Multipart;
        self
    }
    fn login(mut self) -> Self {
        self.special = Special::Login;
        self
    }
    fn logout(mut self) -> Self {
        self.special = Special::Logout;
        self
    }
    fn client_filter(mut self) -> Self {
        self.special = Special::ClientFilter;
        self
    }
}

// ---- terse param constructors ----
fn req(name: &str, ty: PType, desc: &str) -> Param {
    Param { name: name.into(), ty, required: true, fixed: None, desc: desc.into() }
}
fn opt(name: &str, ty: PType, desc: &str) -> Param {
    Param { name: name.into(), ty, required: false, fixed: None, desc: desc.into() }
}
fn konst(name: &str, ty: PType, value: &str, desc: &str) -> Param {
    Param { name: name.into(), ty, required: false, fixed: Some(value.into()), desc: desc.into() }
}

fn cmd(name: &str, path: &str, auth: bool, desc: &str, params: Vec<Param>) -> Command {
    Command {
        name: name.into(),
        aliases: Vec::new(),
        path: path.into(),
        method: Method::Json,
        auth,
        params,
        desc: desc.into(),
        special: Special::None,
    }
}

/// A local-only command (no HTTP) for inspecting saved auth.
fn status_cmd() -> Command {
    Command {
        name: "auth.status".into(),
        aliases: vec!["whoami".into()],
        path: String::new(),
        method: Method::Json,
        auth: false,
        params: vec![],
        desc: "Show the saved login (email, token presence, config path, API URL)".into(),
        special: Special::Status,
    }
}

/// Note appended to endpoints whose body fields aren't fully enumerable; the CLI
/// lets you pass any `--key value` (and `--json '{...}'`) which becomes a body field.
const PASS: &str = "Accepts arbitrary --key value fields (passthrough).";

/// A local-only command that self-updates the binary from GitHub Releases.
fn update_cmd() -> Command {
    Command {
        name: "self.update".into(),
        aliases: vec!["update".into()],
        path: String::new(),
        method: Method::Json,
        auth: false,
        params: vec![
            opt("check", PType::Bool, "Only check whether a newer version exists; don't install"),
            opt("force", PType::Bool, "Reinstall the latest release even if already up to date"),
            opt("insecure", PType::Bool, "Skip SHA-256 checksum verification of the download"),
        ],
        desc: "Check GitHub Releases and upgrade gfit-cli in place".into(),
        special: Special::Update,
    }
}

pub fn registry() -> Vec<Command> {
    let mut c: Vec<Command> = Vec::new();

    // =========================================================== auth
    c.push(
        cmd(
            "auth.login",
            "auth/login",
            false,
            "Log in and save the token to ~/.config/gfit.json. With no --email/--password, opens a browser sign-in page; pass both to log in directly (scriptable)",
            vec![
                opt("email", PType::Str, "Account email (omit to sign in via the browser)"),
                opt("password", PType::Str, "Account password (omit to sign in via the browser)"),
            ],
        )
        .login(),
    );
    c.push(
        cmd("auth.logout", "auth/logout", true, "Log out and clear the saved token", vec![])
            .logout(),
    );
    c.push(cmd("auth.info", "admin/home/info", true, "Get the logged-in user's account info", vec![]));
    c.push(status_cmd());
    c.push(update_cmd());

    // =========================================================== admin: home / clients / users
    c.push(cmd("admin.home", "admin/workout/tool/home", true, "Admin dashboard summary (client counts etc.)", vec![]));
    c.push(cmd(
        "admin.clients",
        "admin/workout/tool/clients",
        true,
        "List workout clients filtered by keyword",
        vec![
            opt("keyword", PType::Str, "Search keyword"),
            konst("all", PType::Bool, "false", "Include all clients regardless of scope"),
        ],
    ));
    c.push(cmd(
        "admin.user-plans",
        "admin/workout/plans",
        true,
        "Get a user's last 10 workout plans",
        vec![req("uid", PType::Num, "User id")],
    ));
    c.push(
        cmd(
            "admin.users",
            "admin/user/list",
            true,
            "List users (type: client | workout | all) with pagination + name filter",
            vec![
                konst("type", PType::Str, "all", "User type: client, workout, or all"),
                opt("page", PType::Num, "Page number"),
                opt("name", PType::Str, "Name filter"),
            ],
        )
        .alias("admin.members"),
    );
    c.push(cmd(
        "admin.user-change-workout",
        "admin/user/change/workout",
        true,
        &format!("Change a user's workout status. {PASS}"),
        vec![opt("uid", PType::Num, "User id")],
    ));
    c.push(cmd(
        "admin.client-update",
        "admin/workout/tool/client/update",
        true,
        &format!("Update client info (nutrition, phase, etc.). {PASS}"),
        vec![
        req("id", PType::Num, "Client id"),
        opt("email", PType::Str, "Client email address"),
        opt("gender", PType::Str, "Gender"),
        opt("birthday", PType::Str, "Date of birth, format YYYY-MM-DD"),
        opt("height", PType::Num, "Height (cm)"),
        opt("weight", PType::Num, "Weight"),
        opt("fitness_location", PType::Str, "Fitness location"),
        opt("workout_month_type", PType::Str, "Workout month type / plan cadence"),
        opt("nickname", PType::Str, "Display nickname"),
        opt("first_name", PType::Str, "First name"),
        opt("last_name", PType::Str, "Last name"),
        opt("is_auto_generate", PType::Bool, "Auto-generate the workout plan"),
        opt("is_upload_app", PType::Bool, "Auto-upload the workout to app"),
        opt("is_send_email", PType::Bool, "Auto-send the workout via email"),
        opt("phase", PType::Str, "Workout phase name (nullable)"),
        opt("nutrition_phase", PType::Str, "Nutrition phase name (nullable)"),
        opt("phase_date", PType::Str, "Phase start date, format YYYY-MM-DD (nullable)"),
        opt("nutrition_activity_level", PType::Num, "Nutrition activity level"),
        opt("nutrition_protein_ratio", PType::Num, "Protein ratio"),
        opt("nutrition_fat_percent", PType::Num, "Fat percent"),
        opt("nutrition_calorie_adj", PType::Num, "Calorie adjustment"),
        opt("nutrition_free_meal_cal", PType::Num, "Free meal calories"),
        opt("daily_calories", PType::Num, "Daily calorie target"),
        opt("daily_protein", PType::Num, "Daily protein target"),
        opt("daily_crab", PType::Num, "Daily carbs target (legacy 'crab' key)"),
        opt("daily_fat", PType::Num, "Daily fat target"),
    ],
    ));
    c.push(cmd(
        "admin.client-create",
        "admin/workout/tool/client/create",
        true,
        &format!("Create a new client. {PASS}"),
        vec![
        req("email", PType::Str, "Client email address"),
        req("gender", PType::Str, "Gender"),
        opt("birthday", PType::Str, "Date of birth, format YYYY-MM-DD"),
        opt("height", PType::Num, "Height (cm)"),
        opt("weight", PType::Num, "Weight"),
        req("fitness_location", PType::Str, "Fitness location"),
        req("workout_month_type", PType::Str, "Workout month type / plan cadence"),
        req("nickname", PType::Str, "Display nickname"),
        req("first_name", PType::Str, "First name"),
        req("last_name", PType::Str, "Last name"),
        req("is_auto_generate", PType::Bool, "Auto-generate the workout plan"),
        req("is_upload_app", PType::Bool, "Auto-upload the workout to app"),
        req("is_send_email", PType::Bool, "Auto-send the workout via email"),
        opt("phase", PType::Str, "Workout phase name (nullable)"),
        opt("nutrition_phase", PType::Str, "Nutrition phase name (nullable)"),
        opt("phase_date", PType::Str, "Phase start date, format YYYY-MM-DD (nullable)"),
        opt("nutrition_activity_level", PType::Num, "Nutrition activity level"),
        opt("nutrition_protein_ratio", PType::Num, "Protein ratio"),
        opt("nutrition_fat_percent", PType::Num, "Fat percent"),
        opt("nutrition_calorie_adj", PType::Num, "Calorie adjustment"),
        opt("nutrition_free_meal_cal", PType::Num, "Free meal calories"),
        opt("daily_calories", PType::Num, "Daily calorie target"),
        opt("daily_protein", PType::Num, "Daily protein target"),
        opt("daily_crab", PType::Num, "Daily carbs target (legacy 'crab' key)"),
        opt("daily_fat", PType::Num, "Daily fat target"),
    ],
    ));
    c.push(cmd(
        "admin.client-coach-update",
        "admin/user/coach/update",
        true,
        "Assign/change a client's coach",
        vec![req("uid", PType::Num, "Client user id"), req("coach_id", PType::Num, "Coach id")],
    ));
    c.push(cmd(
        "admin.coaches",
        "admin/coaches/list",
        true,
        "List coaches",
        vec![konst("type", PType::Str, "coach", "Role type")],
    ));
    c.push(cmd(
        "admin.coach-clients",
        "admin/workout/tool/coach/clients",
        true,
        "List a coach's clients",
        vec![req("coach_id", PType::Num, "Coach id")],
    ));

    // =========================================================== admin: config
    c.push(cmd(
        "admin.config",
        "admin/config/list",
        true,
        "Get config entries by type (main_type=workout_generator)",
        vec![
            req("type", PType::Str, "Config type"),
            konst("main_type", PType::Str, "workout_generator", "Config main type"),
        ],
    ));

    // =========================================================== admin: nutrition meal plans
    let admin_meal = [
        ("admin.meal.list", "admin/nutrition/client/plan/list", "List a client's meal plans", vec![req("client_id", PType::Num, "Client id")]),
        ("admin.meal.generate", "admin/nutrition/client/plan/generate", "Generate a meal plan for a month", vec![req("client_id", PType::Num, "Client id"), req("month", PType::Str, "Month (e.g. 2026-05)")]),
        ("admin.meal.update", "admin/nutrition/client/plan/update", "Update meal plan content", vec![req("uuid", PType::Str, "Plan uuid"), req("content", PType::Json, "Plan content as a JSON array/object")]),
        ("admin.meal.update-targets", "admin/nutrition/client/plan/update-targets", "Update meal plan macro targets", vec![req("uuid", PType::Str, "Plan uuid"), req("targets", PType::Json, "Macro targets as a JSON object")]),
        ("admin.meal.duplicate", "admin/nutrition/client/plan/duplicate", "Duplicate a meal plan to another month", vec![req("uuid", PType::Str, "Plan uuid"), req("month", PType::Str, "Target month")]),
        ("admin.meal.delete", "admin/nutrition/client/plan/delete", "Delete a meal plan", vec![req("uuid", PType::Str, "Plan uuid")]),
        ("admin.meal.preview", "admin/nutrition/client/plan/preview", "Preview a meal plan", vec![req("uuid", PType::Str, "Plan uuid")]),
        ("admin.meal.publish", "admin/nutrition/client/plan/publish", "Publish a meal plan", vec![req("uuid", PType::Str, "Plan uuid")]),
    ];
    for (n, p, d, params) in admin_meal {
        c.push(cmd(n, p, true, d, params));
    }
    c.push(cmd(
        "admin.meal.create",
        "admin/nutrition/client/plan/create",
        true,
        "Create a new meal plan",
        vec![
            req("client_id", PType::Num, "Client id"),
            req("month", PType::Str, "Month"),
            opt("days", PType::Num, "Number of days"),
            opt("meals_per_day", PType::Num, "Meals per day"),
        ],
    ));

    // =========================================================== admin: nutrition food split
    c.push(cmd("admin.food-split.list", "admin/nutrition/split/list", true, "List food splits", vec![]));
    c.push(cmd("admin.food-split.update", "admin/nutrition/split/update", true, &format!("Add/update a food split. {PASS}"), vec![
        opt("id", PType::Num, "Split ID (omit to create new)"),
        opt("name", PType::Str, "Split name"),
        opt("type", PType::Str, "Split type"),
        opt("days", PType::Num, "Number of days in the split"),
        opt("gender", PType::Str, "Target gender"),
        opt("phase", PType::Str, "Phase label (nullable)"),
        opt("description", PType::Str, "Description (nullable)"),
        opt("coach_id", PType::Num, "Owning coach ID (nullable)"),
    ]));
    c.push(cmd("admin.food-split.delete", "admin/nutrition/split/delete", true, &format!("Delete a food split. {PASS}"), vec![
        req("id", PType::Num, "Food split ID"),
    ]));
    c.push(cmd("admin.food-split.details", "admin/nutrition/split/details", true, "Get food split details", vec![req("food_split_id", PType::Num, "Food split id")]));
    c.push(cmd("admin.food-split.details-update", "admin/nutrition/split/details/update", true, &format!("Update/create a food split detail. {PASS}"), vec![
        opt("id", PType::Num, "Detail ID (omit to create new)"),
        opt("food_split_id", PType::Num, "Parent food split ID"),
        opt("day_number", PType::Str, "Day identifier (e.g. Day 1)"),
        opt("day_name", PType::Str, "Day name/label"),
        opt("part", PType::Str, "JSON string of the day's meal parts"),
    ]));
    c.push(cmd("admin.food-split.details-delete", "admin/nutrition/split/details/delete", true, &format!("Delete a food split detail. {PASS}"), vec![
        req("id", PType::Num, "Food split detail ID"),
    ]));
    c.push(cmd("admin.food-split.attach", "admin/nutrition/split/client/attach", true, &format!("Attach a client to a food split. {PASS}"), vec![
        req("food_split_id", PType::Num, "Food split ID"),
        req("client_id", PType::Num, "Client (member) user ID to attach"),
    ]));

    // =========================================================== admin: food database
    c.push(cmd("admin.food.list", "admin/nutrition/food/list", true, "List all foods", vec![]));
    c.push(cmd(
        "admin.food.search",
        "admin/nutrition/food/search",
        true,
        "Search foods by keyword / category / sub-category",
        vec![
            opt("keyword", PType::Str, "Search keyword"),
            opt("category", PType::Str, "Category"),
            opt("sub_category", PType::Str, "Sub-category"),
        ],
    ));
    c.push(cmd("admin.food.categories", "admin/nutrition/food/categories", true, "Get categories and sub-categories", vec![]));
    c.push(cmd("admin.food.create", "admin/nutrition/food/create", true, &format!("Create a food item. {PASS}"), vec![
        req("name", PType::Str, "Food name"),
        req("weight", PType::Num, "Default serving weight in grams"),
        req("measure", PType::Str, "Measure label (e.g. cup, slice)"),
        req("protein", PType::Num, "Protein grams"),
        req("fat", PType::Num, "Fat grams"),
        req("carbs", PType::Num, "Carbohydrate grams"),
        req("calories", PType::Num, "Calories"),
        req("fibre", PType::Num, "Fibre grams"),
        req("category", PType::Str, "Food category"),
        opt("sub_category", PType::Str, "Food sub-category (nullable)"),
        opt("coach_id", PType::Num, "Owning coach ID (null = global)"),
        opt("is_show", PType::Num, "Visibility flag 0/1"),
    ]));
    c.push(cmd("admin.food.update", "admin/nutrition/food/update", true, &format!("Update a food item. {PASS}"), vec![
        req("id", PType::Num, "Food item ID"),
        opt("name", PType::Str, "Food name"),
        opt("weight", PType::Num, "Default serving weight in grams"),
        opt("measure", PType::Str, "Measure label (e.g. cup, slice)"),
        opt("protein", PType::Num, "Protein grams"),
        opt("fat", PType::Num, "Fat grams"),
        opt("carbs", PType::Num, "Carbohydrate grams"),
        opt("calories", PType::Num, "Calories"),
        opt("fibre", PType::Num, "Fibre grams"),
        opt("category", PType::Str, "Food category"),
        opt("sub_category", PType::Str, "Food sub-category (nullable)"),
        opt("coach_id", PType::Num, "Owning coach ID (null = global)"),
        opt("is_show", PType::Num, "Visibility flag 0/1"),
    ]));
    c.push(cmd("admin.food.delete", "admin/nutrition/food/delete", true, &format!("Delete a food item. {PASS}"), vec![
        req("id", PType::Num, "Food item ID"),
    ]));

    // =========================================================== admin: nutrition levers
    c.push(cmd("admin.lever.list", "admin/nutrition/lever/list", true, "List nutrition levers", vec![]));
    c.push(cmd("admin.lever.update", "admin/nutrition/lever/update", true, &format!("Add/update a nutrition lever. {PASS}"), vec![]));

    // =========================================================== admin: video
    c.push(cmd("admin.video.list", "admin/video/workout/list", true, "List workout videos", vec![]));
    c.push(cmd("admin.video.options", "admin/video/workout/options", true, "Get video workout options", vec![]));
    c.push(cmd("admin.video.create", "admin/video/workout/create", true, &format!("Create a video workout entry. {PASS}"), vec![
        req("gender", PType::Str, "Target gender (Both, Male, Female)"),
        req("location", PType::Str, "Home, Gym, or Both"),
        opt("dumbbells", PType::Num, "0/1 uses dumbbells"),
        opt("barbells", PType::Num, "0/1 uses barbells"),
        opt("is_dumbbell_final", PType::Num, "0/1 dumbbell-final flag"),
        opt("is_circuit", PType::Num, "0/1 circuit flag"),
        req("group_name", PType::Str, "Group name"),
        req("part_name", PType::Str, "Body part name"),
        req("exercise_name", PType::Str, "Exercise name"),
        opt("details", PType::Str, "Isolation or Compound"),
        req("video_link", PType::Str, "Video URL"),
        opt("final_video_link", PType::Str, "Final video URL"),
        opt("beginner_friendly", PType::Num, "0/1 beginner-friendly"),
        opt("is_show", PType::Num, "0/1 visibility"),
        opt("reps", PType::Str, "REP1-REP4 (REP1:6-10, REP2:8-12, REP3:10-15, REP4:15-20)"),
    ]));
    c.push(cmd("admin.video.update", "admin/video/workout/update", true, &format!("Update a video workout entry. {PASS}"), vec![
        req("id", PType::Num, "Video record ID"),
        opt("gender", PType::Str, "Target gender (Both, Male, Female)"),
        opt("location", PType::Str, "Home, Gym, or Both"),
        opt("dumbbells", PType::Num, "0/1 uses dumbbells"),
        opt("barbells", PType::Num, "0/1 uses barbells"),
        opt("is_dumbbell_final", PType::Num, "0/1 dumbbell-final flag"),
        opt("is_circuit", PType::Num, "0/1 circuit flag"),
        opt("group_name", PType::Str, "Group name"),
        opt("part_name", PType::Str, "Body part name"),
        opt("exercise_name", PType::Str, "Exercise name"),
        opt("details", PType::Str, "Isolation or Compound"),
        opt("video_link", PType::Str, "Video URL"),
        opt("final_video_link", PType::Str, "Final video URL"),
        opt("beginner_friendly", PType::Num, "0/1 beginner-friendly"),
        opt("is_show", PType::Num, "0/1 visibility"),
        opt("video_id", PType::Num, "External video ID"),
        opt("reps", PType::Str, "REP1-REP4"),
    ]));

    // =========================================================== admin: workout plans
    c.push(cmd("admin.workout.list", "admin/workout/client/plan/list", true, "List a client's workout plans", vec![req("client_id", PType::Num, "Client id")]));
    c.push(cmd("admin.workout.generate", "admin/workout/client/plan/generate", true, "Generate a workout plan for a month", vec![req("client_id", PType::Num, "Client id"), req("month", PType::Str, "Month")]));
    c.push(cmd("admin.workout.publish", "admin/workout/client/plan/publish", true, "Publish a workout plan", vec![req("uuid", PType::Str, "Plan uuid")]));
    c.push(cmd("admin.workout.notice", "admin/workout/client/plan/notice", true, "Send a workout plan notice (by uuid)", vec![req("uuid", PType::Str, "Plan uuid")]));
    c.push(cmd("admin.workout.notice-id", "admin/workout/notice", true, "Send a workout plan notice (by id)", vec![req("id", PType::Num, "Plan id")]));
    c.push(cmd("admin.workout.videos", "admin/workout/client/plan/videos", true, "List videos for a workout part", vec![req("part_name", PType::Str, "Body part"), req("gender", PType::Str, "Gender")]));
    c.push(cmd("admin.workout.video-plan", "admin/workout/client/plan/video/plan", true, "Get exercise-selection plan for a video", vec![req("video_id", PType::Num, "Video id"), req("part_name", PType::Str, "Body part"), req("gender", PType::Str, "Gender")]));
    c.push(cmd("admin.workout.update", "admin/workout/client/plan/update", true, "Update workout plan content", vec![req("uuid", PType::Str, "Plan uuid"), req("content", PType::Json, "Plan content as a JSON array/object")]));
    c.push(cmd("admin.workout.duplicate", "admin/workout/client/plan/duplicate", true, "Duplicate a workout plan to another month", vec![req("uuid", PType::Str, "Plan uuid"), req("month", PType::Str, "Target month")]));
    c.push(cmd("admin.workout.delete", "admin/workout/client/plan/delete", true, "Delete a workout plan", vec![req("uuid", PType::Str, "Plan uuid")]));

    // =========================================================== admin: workout splits
    c.push(cmd("admin.split.list", "staff/workout/split/list", true, "List all splits + config", vec![]));
    c.push(cmd("admin.split.update", "admin/workout/split/update", true, &format!("Add/update a split. {PASS}"), vec![
        opt("id", PType::Num, "Split ID (omit to create new)"),
        opt("name", PType::Str, "Split name"),
        opt("day", PType::Num, "Number of days in the split"),
        opt("gender", PType::Str, "Target gender"),
        opt("description", PType::Str, "Split description (may be empty)"),
        opt("is_only_home", PType::Num, "0/1; 1 = home-only split"),
        opt("is_only_gym", PType::Num, "0/1; 1 = gym-only split"),
    ]));
    c.push(cmd("admin.split.delete", "admin/workout/split/delete", true, &format!("Delete a split. {PASS}"), vec![
        req("id", PType::Num, "Split ID"),
    ]));
    c.push(cmd("admin.split.details", "admin/workout/split/details", true, "Get split details", vec![req("split_id", PType::Num, "Split id")]));
    c.push(cmd("admin.split.details-update", "admin/workout/split/details/update", true, &format!("Update/create a split detail. {PASS}"), vec![
        opt("id", PType::Num, "Detail ID (omit to create new)"),
        opt("split_id", PType::Num, "Parent split ID"),
        opt("day_number", PType::Str, "Day identifier (e.g. Day 1, Every 2nd Day)"),
        opt("day_name", PType::Str, "Day name/label"),
        opt("part", PType::Str, "Body part(s) for the day"),
    ]));
    c.push(cmd("admin.split.details-delete", "admin/workout/split/details/delete", true, &format!("Delete a split detail. {PASS}"), vec![
        req("id", PType::Num, "Split detail ID"),
    ]));
    c.push(cmd("admin.split.report", "admin/workout/split/report", true, "Split report with video availability", vec![]));
    c.push(cmd("admin.split.details-attach", "admin/workout/split/details/client/attach", true, &format!("Attach a client to a split detail. {PASS}"), vec![
        req("id", PType::Num, "Split detail ID"),
        req("client_id", PType::Num, "Client (member) user ID to attach"),
    ]));
    c.push(cmd("admin.split.details-detach", "admin/workout/split/details/client/detach", true, &format!("Detach a client from a split detail. {PASS}"), vec![
        req("id", PType::Num, "Split detail ID"),
        req("client_id", PType::Num, "Client (member) user ID to detach"),
    ]));
    c.push(cmd("admin.split.attach", "admin/workout/split/client/attach", true, &format!("Attach a client to a split. {PASS}"), vec![
        req("split_id", PType::Num, "Split ID"),
        req("client_id", PType::Num, "Client (member) user ID to attach"),
    ]));
    c.push(cmd("admin.split.detach", "admin/workout/split/client/detach", true, &format!("Detach a client from a split. {PASS}"), vec![
        req("split_id", PType::Num, "Split ID"),
        req("client_id", PType::Num, "Client (member) user ID to detach"),
    ]));

    // =========================================================== admin: image upload (multipart)
    c.push(
        cmd(
            "admin.upload",
            "staff/image/upload",
            true,
            "Upload an image (multipart). Use --file <path> --type <type>",
            vec![
                req("file", PType::Str, "Local path to the image file"),
                req("type", PType::Str, "Upload type/category"),
            ],
        )
        .multipart(),
    );

    // =========================================================== admin: nowledge
    c.push(cmd("admin.nowledge.status", "admin/nowledge/status", true, "Nowledge service status/health", vec![]));
    c.push(cmd("admin.nowledge.documents", "admin/nowledge/documents/list", true, "List company documents/context nodes", vec![opt("uri", PType::Str, "Filter by uri")]));
    c.push(cmd("admin.nowledge.sources", "admin/nowledge/documents/sources", true, "Canonical source registry with revision metadata", vec![]));
    c.push(cmd("admin.nowledge.source", "admin/nowledge/documents/source", true, "Fetch a single source's active-revision content", vec![req("source_id", PType::Str, "Source id")]));
    c.push(cmd("admin.nowledge.revisions", "admin/nowledge/documents/revisions", true, "List revisions for a source", vec![req("source_id", PType::Str, "Source id")]));
    c.push(cmd("admin.nowledge.preflight", "admin/nowledge/documents/preflight", true, &format!("Duplicate/overlap check before saving. {PASS}"), vec![
        opt("title", PType::Str, "Document title"),
        opt("text_preview", PType::Str, "Text preview"),
        opt("similarity_threshold", PType::Num, "Similarity threshold"),
        opt("scope", PType::Str, "Scope"),
    ]));
    c.push(cmd("admin.nowledge.create", "admin/nowledge/documents/create", true, &format!("Create a document (preflight + revision). {PASS}"), vec![
        req("title", PType::Str, "Document title"),
        req("content", PType::Str, "Document content"),
    ]));
    c.push(cmd("admin.nowledge.update", "admin/nowledge/documents/update", true, &format!("Update a document (new revision). {PASS}"), vec![
        req("source_id", PType::Str, "Source id"),
        req("title", PType::Str, "Document title"),
        req("content", PType::Str, "Document content"),
    ]));
    c.push(cmd("admin.nowledge.reindex", "admin/nowledge/documents/reindex", true, "Re-fragment a document", vec![req("source_id", PType::Str, "Source id")]));
    c.push(cmd("admin.nowledge.delete", "admin/nowledge/documents/delete", true, "Delete a source document (cascades fragments/revisions)", vec![req("source_id", PType::Str, "Source id")]));
    c.push(cmd("admin.nowledge.search", "admin/nowledge/context/search", true, "Search context (RAG-style hits)", vec![
        req("query", PType::Str, "Search query"),
        konst("limit", PType::Num, "5", "Max hits"),
    ]));
    c.push(cmd("admin.nowledge.title", "admin/nowledge/title/generate", true, &format!("Generate an AI title from content. {PASS}"), vec![req("content", PType::Str, "Content")]));
    c.push(cmd("admin.nowledge.answer", "admin/nowledge/rag/answer", true, &format!("Get a RAG answer with citations. {PASS}"), vec![req("question", PType::Str, "Question")]));
    c.push(cmd("admin.nowledge.proxy", "admin/nowledge/proxy", true, &format!("Generic Nowledge proxy. {PASS}"), vec![
        req("method", PType::Str, "HTTP method"),
        req("path", PType::Str, "Nowledge path"),
    ]));

    // =========================================================== plan meals (staff/plan/meals — uploaded meal plans)
    c.push(cmd("plan.meals", "staff/plan/meals", true, "List a member's (uploaded) meal plans", vec![req("uid", PType::Num, "User id")]));
    c.push(cmd("plan.meals.update", "staff/plan/meals/update", true, &format!("Update an uploaded meal plan. {PASS}"), vec![
        opt("type", PType::Str, "Type"),
        opt("uid", PType::Num, "User id"),
        opt("date", PType::Str, "Date"),
        opt("url", PType::Str, "File url"),
        opt("id", PType::Num, "Record id"),
    ]));
    c.push(cmd("plan.meals.notice", "staff/plan/meals/notice", true, "Send a meal plan notice email", vec![req("id", PType::Num, "Record id")]));

    // =========================================================== coach: members
    c.push(
        cmd("coach.clients", "staff/workout/client/list", true, "List the coach's assigned clients (--search filters client-side)", vec![
            opt("search", PType::Str, "Filter returned clients by name/email (client-side)"),
        ])
        .client_filter(),
    );
    c.push(cmd("coach.client-update", "staff/workout/client/update", true, &format!("Update client info (partial). {PASS}"), vec![
        req("id", PType::Num, "client id"),
        opt("email", PType::Str, "client email"),
        opt("gender", PType::Str, "client gender"),
        opt("fitness_location", PType::Str, "fitness location"),
        opt("workout_type", PType::Str, "workout type (nullable)"),
        opt("workout_month_type", PType::Str, "workout month type"),
        opt("first_name", PType::Str, "first name"),
        opt("last_name", PType::Str, "last name"),
        opt("nickname", PType::Str, "nickname"),
        opt("is_auto_generate", PType::Bool, "auto-generate flag (boolean or 0/1)"),
        opt("is_upload_app", PType::Bool, "upload-to-app flag (boolean or 0/1)"),
        opt("is_send_email", PType::Bool, "send-email flag (boolean or 0/1)"),
        konst("weight", PType::Num, "0", "weight, multiply 10 from actual weight (kg), default 0"),
        opt("height", PType::Num, "actual height (cm), default null"),
        opt("birthday", PType::Str, "format YYYY-MM-DD, default null"),
        opt("split_date", PType::Str, "format YYYY-MM-DD, current split working date"),
        opt("split_level_date", PType::Str, "format YYYY-MM-DD, current phase working date"),
        opt("split_level", PType::Num, "split level"),
        opt("phase", PType::Str, "phase name"),
        opt("nutrition_phase", PType::Str, "nutrition phase name"),
        opt("phase_date", PType::Str, "format YYYY-MM-DD, current phase start date (set to now when phase changes)"),
        opt("goal", PType::Str, "goal name"),
        konst("init_weight", PType::Num, "0", "initial weight, multiply 10 from actual weight (kg), default 0"),
        opt("last_meal_date", PType::Str, "format YYYY-MM-DD, last meal date"),
        opt("last_generate_date", PType::Str, "format YYYY-MM-DD, last generate date"),
        opt("note", PType::Str, "note for the client"),
    ]));
    c.push(cmd("coach.publish-date", "staff/workout/publish/update", true, "Update a client's publish date", vec![req("uid", PType::Num, "User id"), req("date", PType::Str, "Date")]));
    c.push(cmd("coach.search", "staff/member/search", true, "Search a member by name/email", vec![req("name", PType::Str, "Name or email")]));
    c.push(cmd("coach.attach", "staff/member/attach", true, "Add a client to the coach", vec![req("uid", PType::Str, "User id")]));
    c.push(cmd("coach.detach", "staff/member/detach", true, "Remove a client from the coach", vec![req("uid", PType::Str, "User id")]));
    c.push(cmd("coach.create", "staff/member/create", true, &format!("Create a new client. {PASS}"), vec![
        req("email", PType::Str, "Email"),
        req("first_name", PType::Str, "First name"),
        req("last_name", PType::Str, "Last name"),
        opt("gender", PType::Str, "Gender"),
        opt("fitness_location", PType::Str, "Fitness location"),
        opt("workout_month_type", PType::Str, "Workout month type"),
        opt("weight", PType::Num, "Weight"),
        opt("height", PType::Num, "Height"),
        opt("birthday", PType::Str, "Birthday"),
    ]));
    c.push(cmd("coach.profile-history", "staff/client/profile/history", true, "Get a client's profile property change history", vec![req("uid", PType::Num, "User id"), req("property", PType::Str, "Property name")]));
    c.push(cmd("coach.info", "staff/home/info", true, "Coach dashboard info", vec![]));
    c.push(cmd("coach.merge-user", "staff/workout/client/merge-user", true, "Merge a source user into a target user", vec![req("target_uid", PType::Num, "Target user id"), req("source_uid", PType::Num, "Source user id")]));

    // =========================================================== coach: emails
    c.push(cmd("coach.email.check", "staff/client/emails/check", true, "Check if an email exists / belongs to a user", vec![req("email", PType::Str, "Email")]));
    c.push(cmd("coach.email.list", "staff/client/emails", true, "List a member's secondary emails", vec![req("uid", PType::Num, "User id")]));
    c.push(cmd("coach.email.create", "staff/client/emails/create", true, "Add a member secondary email", vec![req("uid", PType::Num, "User id"), req("email", PType::Str, "Email")]));
    c.push(cmd("coach.email.set-primary", "staff/client/emails/set-primary", true, "Set a member's primary email", vec![req("uid", PType::Num, "User id"), req("email", PType::Str, "Email")]));
    c.push(cmd("coach.email.delete", "staff/client/emails/delete", true, "Delete a member secondary email", vec![req("uid", PType::Num, "User id"), req("email", PType::Str, "Email")]));

    // =========================================================== coach: nutrition meal plans
    let coach_meal = [
        ("coach.meal.list", "staff/nutrition/client/plan/list", "List a client's meal plans", vec![req("client_id", PType::Num, "Client id")]),
        ("coach.meal.generate", "staff/nutrition/client/plan/generate", "Generate a meal plan for a month", vec![req("client_id", PType::Num, "Client id"), req("month", PType::Str, "Month")]),
        ("coach.meal.update", "staff/nutrition/client/plan/update", "Update meal plan content", vec![req("uuid", PType::Str, "Plan uuid"), req("content", PType::Json, "Plan content as a JSON array/object")]),
        ("coach.meal.duplicate", "staff/nutrition/client/plan/duplicate", "Duplicate a meal plan to another month", vec![req("uuid", PType::Str, "Plan uuid"), req("month", PType::Str, "Target month")]),
        ("coach.meal.delete", "staff/nutrition/client/plan/delete", "Delete a meal plan", vec![req("uuid", PType::Str, "Plan uuid")]),
        ("coach.meal.preview", "staff/nutrition/client/plan/preview", "Preview a meal plan", vec![req("uuid", PType::Str, "Plan uuid")]),
        ("coach.meal.publish", "staff/nutrition/client/plan/publish", "Publish a meal plan", vec![req("uuid", PType::Str, "Plan uuid")]),
    ];
    for (n, p, d, params) in coach_meal {
        c.push(cmd(n, p, true, d, params));
    }
    c.push(cmd("coach.meal.create", "staff/nutrition/client/plan/create", true, "Create a new meal plan", vec![
        req("client_id", PType::Num, "Client id"),
        req("month", PType::Str, "Month"),
        opt("days", PType::Num, "Number of days"),
        opt("meals_per_day", PType::Num, "Meals per day"),
    ]));

    // =========================================================== coach: nutrition food split
    c.push(cmd("coach.food-split.list", "staff/nutrition/split/list", true, "List food splits", vec![]));
    c.push(cmd("coach.food-split.attach", "staff/nutrition/split/client/attach", true, &format!("Attach a client to a food split. {PASS}"), vec![
        req("food_split_id", PType::Num, "Food split id to assign"),
        req("client_id", PType::Num, "Client user ID being assigned"),
    ]));

    // =========================================================== coach: workout splits
    c.push(cmd("coach.split.list", "staff/workout/split/list", true, "List splits available to coaches", vec![]));
    c.push(cmd("coach.split.details", "staff/workout/split/details", true, "Get split details", vec![req("split_id", PType::Num, "Split id")]));
    c.push(cmd("coach.split.attach", "staff/workout/split/client/attach", true, &format!("Attach a client to a split. {PASS}"), vec![
        req("split_id", PType::Num, "Workout split id to assign"),
        req("client_id", PType::Num, "Client user ID being assigned"),
    ]));
    c.push(cmd("coach.split.detach", "staff/workout/split/client/detach", true, &format!("Detach a client from a split. {PASS}"), vec![
        req("split_id", PType::Num, "Workout split id to unassign"),
        req("client_id", PType::Num, "Client user ID being unassigned"),
    ]));

    // =========================================================== coach: workout plans
    c.push(cmd("coach.workout.plans", "staff/workout/plans", true, "Get a user's last 10 workout plans", vec![req("uid", PType::Num, "User id")]));
    c.push(cmd("coach.workout.notice-id", "staff/workout/notice", true, "Send a workout plan notice (by id)", vec![req("id", PType::Num, "Plan id")]));
    c.push(cmd("coach.workout.list", "staff/workout/client/plan/list", true, "List a client's workout plans", vec![req("client_id", PType::Num, "Client id")]));
    c.push(cmd("coach.workout.generate", "staff/workout/client/plan/generate", true, "Generate a workout plan for a month", vec![req("client_id", PType::Num, "Client id"), req("month", PType::Str, "Month")]));
    c.push(cmd("coach.workout.publish", "staff/workout/client/plan/publish", true, "Publish a workout plan", vec![req("uuid", PType::Str, "Plan uuid")]));
    c.push(cmd("coach.workout.notice", "staff/workout/client/plan/notice", true, "Send a workout plan notice (by uuid)", vec![req("uuid", PType::Str, "Plan uuid")]));
    c.push(cmd("coach.workout.videos", "staff/workout/client/plan/videos", true, "List videos for a workout part", vec![req("part_name", PType::Str, "Body part"), req("gender", PType::Str, "Gender")]));
    c.push(cmd("coach.workout.video-plan", "staff/workout/client/plan/video/plan", true, "Get exercise-selection plan for a video", vec![req("video_id", PType::Num, "Video id"), req("part_name", PType::Str, "Body part"), req("gender", PType::Str, "Gender")]));
    c.push(cmd("coach.workout.update", "staff/workout/client/plan/update", true, "Update workout plan content", vec![req("uuid", PType::Str, "Plan uuid"), req("content", PType::Json, "Plan content as a JSON array/object")]));
    c.push(cmd("coach.workout.duplicate", "staff/workout/client/plan/duplicate", true, "Duplicate a workout plan to another month", vec![req("uuid", PType::Str, "Plan uuid"), req("month", PType::Str, "Target month")]));
    c.push(cmd("coach.workout.delete", "staff/workout/client/plan/delete", true, "Delete a workout plan", vec![req("uuid", PType::Str, "Plan uuid")]));

    // =========================================================== coach: check-ins
    c.push(cmd("coach.checkin.list", "staff/member/checkin", true, "Get a member's recent check-ins", vec![req("id", PType::Str, "User id")]));
    c.push(cmd("coach.checkin.note", "staff/member/checkin/note", true, "Set a note on a check-in", vec![req("id", PType::Str, "Check-in id"), req("note", PType::Str, "Note")]));
    c.push(cmd("coach.checkin.answer-update", "staff/member/checkin/answer/update", true, "Update a single check-in answer (re-processes metrics)", vec![req("id", PType::Num, "Check-in id"), req("question", PType::Str, "Question key"), req("answer", PType::Str, "New answer")]));

    // =========================================================== coach: weight
    c.push(cmd("coach.weight.info", "staff/client/weight/info", true, "Get a client's weight goal", vec![req("uid", PType::Num, "User id")]));
    c.push(cmd("coach.weight.update", "staff/client/weight/update", true, "Create/update a client's weight goal", vec![
        req("uid", PType::Num, "User id"),
        opt("current_weight", PType::Num, "Current weight"),
        opt("start_weight", PType::Num, "Start weight"),
        opt("goal_weight", PType::Num, "Goal weight"),
        opt("start_date", PType::Str, "Start date"),
        opt("end_date", PType::Str, "End date"),
    ]));
    c.push(cmd("coach.weight.log-update", "staff/client/weight/log/update", true, "Submit a weight log entry", vec![
        req("uid", PType::Num, "User id"),
        req("weight", PType::Num, "Weight"),
        req("date", PType::Str, "Date"),
        opt("note", PType::Str, "Note"),
    ]));
    c.push(cmd("coach.weight.log-list", "staff/client/weight/log/list", true, "List weight log entries in a date range", vec![
        req("uid", PType::Num, "User id"),
        opt("start_date", PType::Str, "Start date"),
        opt("end_date", PType::Str, "End date"),
    ]));
    c.push(cmd("coach.weight.log-delete", "staff/client/weight/log/delete", true, "Delete a weight log entry", vec![req("uid", PType::Num, "User id"), req("id", PType::Num, "Log id")]));

    // =========================================================== coach: step
    c.push(cmd("coach.step.log-update", "staff/client/step/log/update", true, "Create/update a step log entry", vec![
        req("uid", PType::Num, "User id"),
        req("steps", PType::Num, "Steps"),
        req("date", PType::Str, "Date"),
        opt("note", PType::Str, "Note"),
    ]));
    c.push(cmd("coach.step.log-list", "staff/client/step/log/list", true, "List step log entries in a date range", vec![
        req("uid", PType::Num, "User id"),
        opt("start_date", PType::Str, "Start date"),
        opt("end_date", PType::Str, "End date"),
    ]));
    c.push(cmd("coach.step.log-delete", "staff/client/step/log/delete", true, "Delete a step log entry", vec![req("uid", PType::Num, "User id"), req("id", PType::Num, "Log id")]));

    // =========================================================== coach: sleep
    c.push(cmd("coach.sleep.log-update", "staff/client/sleep/log/update", true, "Create/update a sleep log entry", vec![
        req("uid", PType::Num, "User id"),
        req("sleep_time", PType::Num, "Sleep time"),
        opt("efficiency_score", PType::Num, "Efficiency score"),
        req("date", PType::Str, "Date"),
        opt("note", PType::Str, "Note"),
    ]));
    c.push(cmd("coach.sleep.log-list", "staff/client/sleep/log/list", true, "List sleep log entries in a date range", vec![
        req("uid", PType::Num, "User id"),
        opt("start_date", PType::Str, "Start date"),
        opt("end_date", PType::Str, "End date"),
    ]));
    c.push(cmd("coach.sleep.log-delete", "staff/client/sleep/log/delete", true, "Delete a sleep log entry", vec![req("uid", PType::Num, "User id"), req("id", PType::Num, "Log id")]));

    // =========================================================== coach: text logs (stress/dietary/energy)
    c.push(cmd("coach.text.stress", "staff/client/text/log/stress", true, "List stress log entries in a date range", vec![
        req("uid", PType::Num, "User id"),
        opt("start_date", PType::Str, "Start date"),
        opt("end_date", PType::Str, "End date"),
    ]));
    c.push(cmd("coach.text.dietary", "staff/client/text/log/dietary", true, "List dietary log entries in a date range", vec![
        req("uid", PType::Num, "User id"),
        opt("start_date", PType::Str, "Start date"),
        opt("end_date", PType::Str, "End date"),
    ]));
    c.push(cmd("coach.text.energy", "staff/client/text/log/energy", true, "List energy log entries in a date range", vec![
        req("uid", PType::Num, "User id"),
        opt("start_date", PType::Str, "Start date"),
        opt("end_date", PType::Str, "End date"),
    ]));
    c.push(cmd("coach.text.update", "staff/client/text/log/update", true, "Create/update a text log entry", vec![
        req("uid", PType::Num, "User id"),
        req("type", PType::Str, "Log type (stress/dietary/energy)"),
        req("date", PType::Str, "Date"),
        req("score", PType::Num, "Score"),
        opt("value", PType::Str, "Value"),
    ]));

    // =========================================================== coach: invoice
    c.push(cmd("coach.invoice.list", "staff/invoice/list", true, "List a client's invoices (incl. family members)", vec![req("uid", PType::Num, "User id")]));
    c.push(cmd("coach.invoice.download", "staff/invoice/invoice", true, "Get an invoice PDF download URL", vec![req("id", PType::Num, "Invoice id")]));
    c.push(cmd("coach.invoice.dietitian", "staff/invoice/dietitian", true, "Get a dietitian receipt PDF download URL", vec![req("id", PType::Num, "Invoice id")]));

    c
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn registry_is_substantial() {
        assert!(registry().len() >= 140, "expected the full API surface");
    }

    #[test]
    fn command_names_are_unique() {
        let reg = registry();
        let mut seen = HashSet::new();
        for c in &reg {
            assert!(seen.insert(c.name.clone()), "duplicate command name: {}", c.name);
        }
    }

    #[test]
    fn aliases_do_not_collide_with_names_or_each_other() {
        let reg = registry();
        let names: HashSet<&str> = reg.iter().map(|c| c.name.as_str()).collect();
        let mut all_aliases = HashSet::new();
        for c in &reg {
            for a in &c.aliases {
                assert!(!names.contains(a.as_str()), "alias '{a}' collides with a command name");
                assert!(all_aliases.insert(a.clone()), "duplicate alias: {a}");
            }
        }
    }

    #[test]
    fn user_example_commands_resolve() {
        let reg = registry();
        for name in ["auth.login", "admin.members", "coach.clients"] {
            assert!(reg.iter().any(|c| c.matches(name)), "missing example command: {name}");
        }
    }

    #[test]
    fn only_login_skips_auth_among_http_commands() {
        let reg = registry();
        let login = reg.iter().find(|c| c.special == Special::Login).expect("login command");
        assert_eq!(login.path, "auth/login");
        assert!(!login.auth, "login must not require a token");
    }

    #[test]
    fn http_commands_have_a_path() {
        for c in registry() {
            if !matches!(c.special, Special::Status | Special::Update) {
                assert!(!c.path.is_empty(), "{} has no path", c.name);
            }
        }
    }

    #[test]
    fn coerce_number_int_float_and_error() {
        assert_eq!(PType::Num.coerce("42").unwrap(), json!(42));
        assert_eq!(PType::Num.coerce("3.5").unwrap(), json!(3.5));
        assert!(PType::Num.coerce("abc").is_err());
    }

    #[test]
    fn coerce_bool_accepts_common_spellings() {
        assert_eq!(PType::Bool.coerce("true").unwrap(), json!(true));
        assert_eq!(PType::Bool.coerce("1").unwrap(), json!(true));
        assert_eq!(PType::Bool.coerce("no").unwrap(), json!(false));
        assert!(PType::Bool.coerce("maybe").is_err());
    }

    #[test]
    fn coerce_json_parses_structures() {
        assert_eq!(PType::Json.coerce("[1,2,3]").unwrap(), json!([1, 2, 3]));
        assert_eq!(PType::Json.coerce(r#"{"a":1}"#).unwrap(), json!({"a": 1}));
        assert!(PType::Json.coerce("not json").is_err());
    }

    #[test]
    fn coerce_loose_detects_types() {
        assert_eq!(coerce_loose("true"), json!(true));
        assert_eq!(coerce_loose("7"), json!(7));
        assert_eq!(coerce_loose("1.25"), json!(1.25));
        assert_eq!(coerce_loose("hello"), json!("hello"));
        assert_eq!(coerce_loose("[1,2]"), json!([1, 2]));
        assert_eq!(coerce_loose(r#"{"k":"v"}"#), json!({"k": "v"}));
    }
}

#[cfg(test)]
mod auth_policy_tests {
    use super::*;

    /// Every networked command must require auth EXCEPT `auth.login`. Locks the
    /// "not logged in => only auth.login runs" policy so a future endpoint can't
    /// silently opt out of the login gate.
    #[test]
    fn only_login_runs_without_auth() {
        for c in registry() {
            if matches!(c.special, Special::Status | Special::Update) {
                continue; // local-only command, never networked
            }
            if c.special == Special::Login {
                assert!(!c.auth, "login itself must not require a token");
            } else {
                assert!(c.auth, "{} must require auth (login gate)", c.name);
            }
        }
    }
}
