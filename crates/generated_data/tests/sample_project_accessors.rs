use data_studio_core::{DataProject, RowId};
use generated_data::relation_cache::GeneratedRelationCache;
use generated_data::table_accessors::GeneratedDatabase;

#[test]
fn loads_sample_project_with_generated_accessors() {
    let project =
        DataProject::load_from_dir("../../projects/sample").expect("sample project loads");
    let db = GeneratedDatabase::from_project(&project).expect("generated database loads");

    let knight = db.unit_def.get_by_key("knight").expect("knight exists");
    assert_eq!(knight.max_hp, 120);

    let member = db
        .unit_group_member
        .get_by_id(RowId(5001))
        .expect("party knight member exists");
    assert_eq!(member.unit, RowId(1001));
    assert_eq!(member.lane, -0.6);

    let map = db
        .map_def
        .get_by_key("endless_left_road")
        .expect("map exists");
    assert_eq!(map.waves, vec![RowId(3001), RowId(3002)]);
}

#[test]
fn builds_sample_project_relation_cache() {
    let project =
        DataProject::load_from_dir("../../projects/sample").expect("sample project loads");
    let db = GeneratedDatabase::from_project(&project).expect("generated database loads");
    let cache = GeneratedRelationCache::build(&db).expect("relation cache builds");

    let map = db
        .map_def
        .get_by_key("endless_left_road")
        .expect("map exists");
    assert_eq!(cache.get_map_def_party(map.id), Some(RowId(2001)));
    assert_eq!(
        cache.get_map_def_waves(map.id),
        Some(vec![RowId(3001), RowId(3002)].as_slice())
    );

    let member = db
        .unit_group_member
        .get_by_key("party_start_knight")
        .expect("member exists");
    assert_eq!(
        cache.get_unit_group_member_unit(member.id),
        Some(RowId(1001))
    );
}
