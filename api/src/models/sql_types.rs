use diesel_derive_enum::DbEnum;

#[derive(Copy, Clone, Debug, Eq, Hash, PartialEq, DbEnum)]
#[DieselType = "Permission_type"]
pub enum PermissionType {
    CreateSpecs,
    ModifyAllSpecs,
    DeleteAllSpecs,
}

#[derive(Copy, Clone, Debug, Eq, Hash, PartialEq, DbEnum)]
#[DieselType = "Role_type"]
pub enum RoleType {
    /// These users can perform any action.
    Admin,

    /// These users can create specs. Once we start attaching the creator to
    /// each spec then we can allow these users to modify their own specs, but
    /// for now they can only create them.
    SpecCreator,
}
