use data_studio_core::{DataProject, RowId};
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
