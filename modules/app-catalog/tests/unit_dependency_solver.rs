#![forbid(unsafe_code)]

use app_catalog::dependency_solver::{Dependency, DependencyError, DependencySolver, Package, Version, VersionConstraint};

#[test]
fn test_version_parsing() {
    assert_eq!("1.2.3".parse::<Version>().unwrap(), Version { major: 1, minor: 2, patch: 3 });
    assert_eq!("1.2".parse::<Version>().unwrap(), Version { major: 1, minor: 2, patch: 0 });
    assert_eq!("1".parse::<Version>().unwrap(), Version { major: 1, minor: 0, patch: 0 });

    assert!( "1.2.3.4".parse::<Version>().is_err() );
    assert!( "".parse::<Version>().is_err() );
}

#[test]
fn test_version_constraints() {
    let v1_2_3 = "1.2.3".parse::<Version>().unwrap();
    let v2_0_0 = "2.0.0".parse::<Version>().unwrap();
    let v1_2_0 = "1.2.0".parse::<Version>().unwrap();

    let constraint_ge = ">= 1.2.0".parse::<VersionConstraint>().unwrap();
    assert!(constraint_ge.matches(&v1_2_3));
    assert!(constraint_ge.matches(&v2_0_0));
    assert!(constraint_ge.matches(&v1_2_0));

    let constraint_gt = "> 1.2.0".parse::<VersionConstraint>().unwrap();
    assert!(constraint_gt.matches(&v1_2_3));
    assert!(constraint_gt.matches(&v2_0_0));
    assert!(!constraint_gt.matches(&v1_2_0));

    let constraint_le = "<= 1.2.3".parse::<VersionConstraint>().unwrap();
    assert!(constraint_le.matches(&v1_2_3));
    assert!(!constraint_le.matches(&v2_0_0));
    assert!(constraint_le.matches(&v1_2_0));

    let constraint_lt = "< 2.0.0".parse::<VersionConstraint>().unwrap();
    assert!(constraint_lt.matches(&v1_2_3));
    assert!(!constraint_lt.matches(&v2_0_0));
    assert!(constraint_lt.matches(&v1_2_0));

    let constraint_eq = "= 1.2.3".parse::<VersionConstraint>().unwrap();
    assert!(constraint_eq.matches(&v1_2_3));
    assert!(!constraint_eq.matches(&v2_0_0));
    assert!(!constraint_eq.matches(&v1_2_0));
}

#[test]
fn test_basic_resolution() {
    let mut solver = DependencySolver::new();

    solver.add_package(Package::new("PHP", "8.2.0").unwrap());
    solver.add_package(Package::new("PHP", "8.1.0").unwrap());
    solver.add_package(Package::new("MariaDB", "10.6.0").unwrap());

    let target = Package::new("MyApp", "1.0.0").unwrap()
        .with_dependency(Dependency::new("PHP", ">= 8.1.0").unwrap())
        .with_dependency(Dependency::new("MariaDB", ">= 10.0.0").unwrap());

    solver.add_package(target);

    let resolved = solver.resolve("MyApp", &VersionConstraint::Any).unwrap();

    assert_eq!(resolved.len(), 3);

    // PHP 8.2.0 is selected because it's the highest version that satisfies the constraint
    assert!(resolved.contains(&Package::new("PHP", "8.2.0").unwrap()));
    assert!(resolved.contains(&Package::new("MariaDB", "10.6.0").unwrap()));
}

#[test]
fn test_version_incompatibility() {
    let mut solver = DependencySolver::new();

    solver.add_package(Package::new("PHP", "8.0.0").unwrap());

    let target = Package::new("MyApp", "1.0.0").unwrap()
        .with_dependency(Dependency::new("PHP", ">= 8.2.0").unwrap());

    solver.add_package(target);

    let result = solver.resolve("MyApp", &VersionConstraint::Any);

    assert_eq!(result, Err(DependencyError::VersionIncompatibility("PHP".to_string())));
}

#[test]
fn test_circular_dependency() {
    let mut solver = DependencySolver::new();

    let pkg_a = Package::new("A", "1.0.0").unwrap()
        .with_dependency(Dependency::new("B", ">= 1.0.0").unwrap());

    let pkg_b = Package::new("B", "1.0.0").unwrap()
        .with_dependency(Dependency::new("A", ">= 1.0.0").unwrap());

    solver.add_package(pkg_a);
    solver.add_package(pkg_b);

    let result = solver.resolve("A", &VersionConstraint::Any);

    assert_eq!(result, Err(DependencyError::CircularDependency("A".to_string())));
}

#[test]
fn test_deep_dependencies() {
    let mut solver = DependencySolver::new();

    solver.add_package(Package::new("Redis", "6.0.0").unwrap());
    solver.add_package(Package::new("Redis", "5.0.0").unwrap());

    solver.add_package(Package::new("LibDB", "2.0.0").unwrap()
        .with_dependency(Dependency::new("Redis", ">= 6.0.0").unwrap()));

    solver.add_package(Package::new("LibDB", "1.0.0").unwrap()
        .with_dependency(Dependency::new("Redis", ">= 5.0.0").unwrap()));

    let target = Package::new("App", "1.0.0").unwrap()
        .with_dependency(Dependency::new("LibDB", ">= 2.0.0").unwrap());

    solver.add_package(target);

    let resolved = solver.resolve("App", &VersionConstraint::Any).unwrap();

    assert_eq!(resolved.len(), 3);
    assert!(resolved.iter().any(|p| p.name == "Redis" && p.version == "6.0.0".parse().unwrap()));
    assert!(resolved.iter().any(|p| p.name == "LibDB" && p.version == "2.0.0".parse().unwrap()));
}

#[test]
fn test_shared_dependencies() {
    let mut solver = DependencySolver::new();

    solver.add_package(Package::new("LibCommon", "1.5.0").unwrap());

    solver.add_package(Package::new("LibA", "1.0.0").unwrap()
        .with_dependency(Dependency::new("LibCommon", ">= 1.0.0").unwrap()));

    solver.add_package(Package::new("LibB", "1.0.0").unwrap()
        .with_dependency(Dependency::new("LibCommon", "< 2.0.0").unwrap()));

    let target = Package::new("App", "1.0.0").unwrap()
        .with_dependency(Dependency::new("LibA", ">= 1.0.0").unwrap())
        .with_dependency(Dependency::new("LibB", ">= 1.0.0").unwrap());

    solver.add_package(target);

    let resolved = solver.resolve("App", &VersionConstraint::Any).unwrap();

    assert_eq!(resolved.len(), 4);
    assert!(resolved.iter().any(|p| p.name == "LibCommon" && p.version == "1.5.0".parse().unwrap()));
}

#[test]
fn test_shared_dependencies_conflict() {
    let mut solver = DependencySolver::new();

    solver.add_package(Package::new("LibCommon", "1.5.0").unwrap());
    solver.add_package(Package::new("LibCommon", "2.0.0").unwrap());

    solver.add_package(Package::new("LibA", "1.0.0").unwrap()
        .with_dependency(Dependency::new("LibCommon", "= 1.5.0").unwrap()));

    solver.add_package(Package::new("LibB", "1.0.0").unwrap()
        .with_dependency(Dependency::new("LibCommon", "= 2.0.0").unwrap()));

    let target = Package::new("App", "1.0.0").unwrap()
        .with_dependency(Dependency::new("LibA", ">= 1.0.0").unwrap())
        .with_dependency(Dependency::new("LibB", ">= 1.0.0").unwrap());

    solver.add_package(target);

    let result = solver.resolve("App", &VersionConstraint::Any);

    assert_eq!(result, Err(DependencyError::VersionIncompatibility("LibCommon".to_string())));
}
