mod nqcrc;
mod xorjump;

use std::convert::TryInto;
use bitflags::bitflags;

use nom::{
    branch::alt,
    bytes::{
        complete::{take, take_while},
        streaming::tag,
    },
    character::is_digit,
    combinator::{map, map_res},
    number::complete::{le_f32, le_i32, le_u16, le_u32, le_u8, le_u64},
    sequence::{delimited, preceded, terminated, tuple},
    IResult,
};

#[derive(Debug)]
pub struct Dmb {
    grid: Grid,
    path_table: Vec<Path>,
    mob_table: Vec<Mob>,
    string_table: Vec<DMString>,
    misc_table: Vec<Misc>,
    proc_table: Vec<Proc>,
    variable_table: Vec<Variable>,
    some_proc_table: Vec<ProcId>,
    instance_table: Vec<Instance>,
    map_data_table: Vec<MapData>,
    world: World,
    file_table: Vec<File>,
}

impl Dmb {
    fn string(&self, id: StringId) -> &[u8] {
        let str = &self.string_table[id.0.0 as usize];
        &str.data
    }

    fn proc(&self, id: ProcId) -> &Proc {
        &self.proc_table[id.0.0 as usize]
    }
}

#[derive(Copy, Clone, Debug)]
struct ObjectId(u32);

macro_rules! define_object_kind {
    ($name:ident) => {
        #[derive(Copy, Clone, Debug)]
        struct $name(ObjectId);

        impl From<ObjectId> for $name {
            fn from(id: ObjectId) -> Self {
                Self(id)
            }
        }
    };
}

define_object_kind!(PathId);
define_object_kind!(MobId);
define_object_kind!(StringId);
define_object_kind!(MiscId);
define_object_kind!(ProcId);
define_object_kind!(InstanceId);
define_object_kind!(FileId);

#[derive(Debug)]
struct Grid;

#[derive(Debug)]
struct Path {
    path: StringId,
    parent: Option<PathId>,
    name: Option<StringId>,
    desc: Option<StringId>,
    icon: Option<FileId>,
    icon_state: Option<StringId>,
    direction: u8,
    interface: u32, // TODO: preserve long-ness. Do I care?
    text: Option<StringId>,
    maptext: Option<ObjectId>,
    maptext_width: u16,
    maptext_height: u16,
    maptext_x: u16,
    maptext_y: u16,
    suffix: Option<StringId>,
    flags: u64, // contains (invisibility == 0), mouse_opacity, mouse_drop_zone, density, opacity, gender, override, animate_movement: see <https://github.com/willox/dmdiag/blob/cc5db4496a3c9054a8645cf1c5ce743b89081377/dm/Mob.h#L34-L44>
    verbs: Option<MiscId>,
    procs: Option<MiscId>,
    initializer: Option<ProcId>,
    initialized_vars: Option<MiscId>,
    defining_vars: Option<MiscId>,
    layer: f32,
    transform: Option<[f32; 6]>,
    color_matrix: Option<[f32; 20]>,
    overriding_vars: Option<MiscId>,
}

bitflags! {
    pub struct PathFlags: u64 {
        const OPACITY = 0x01;
        const DENSITY = 0x02;
        const VISIBILITY = 0x04;
        const LUMINOSITY_0 = 0x08;
        const LUMINOSITY_1 = 0x10;
        const LUMINOSITY_2 = 0x20;
        const GENDER_LO = 0x40;
        const GENDER_HI = 0x80;
        const MOUSE_DROP_ZONE = 0x100;
        // = 0x200;
        const ANIMATE_MOVEMENT_DISABLED = 0x400;
        const HAS_MOUSE_PROC = 0x800;
        const MOUSE_OPACITY_LO = 0x1000;
        const MOUSE_OPACITY_HI = 0x2000;
        const ANIMATE_MOVEMENT_LO = 0x4000;
        const ANIMATE_MOVEMENT_HI = 0x8000;
        const TODO_PREFAB_LIKE_UNKNOWN = 0x10000;
        // = 0x20000
        const OVERRIDE = 0x40000;
        const HAS_MOUSE_MOVE_PROC = 0x80000;
        const APPEARANCE_FLAGS_0 = 0x100000;
        const APPEARANCE_FLAGS_1 = 0x200000;
        const APPEARANCE_FLAGS_2 = 0x400000;
        const APPEARANCE_FLAGS_3 = 0x800000;
        const APPEARANCE_FLAGS_4 = 0x1000000;
        const APPEARANCE_FLAGS_5 = 0x2000000;
        const APPEARANCE_FLAGS_6 = 0x4000000;
        const APPEARANCE_FLAGS_7 = 0x8000000;
        const APPEARANCE_FLAGS_8 = 0x10000000;
        const APPEARANCE_FLAGS_9 = 0x20000000;
        const APPEARANCE_FLAGS_A = 0x40000000;
        const APPEARANCE_FLAGS_B = 0x80000000;
        const APPEARANCE_FLAGS_C = 0x100000000;
        const APPEARANCE_FLAGS_D = 0x200000000;
        const APPEARANCE_FLAGS_E = 0x400000000;
        const APPEARANCE_FLAGS_F = 0x800000000;
    }
}

#[derive(Debug)]
struct Mob {
    path: PathId,
    key: Option<StringId>,
    sight_flags: u8,
    sight_flags_ex: Option<u32>,
    see_in_dark: Option<u8>,
    see_invisible: Option<u8>,
}

#[derive(Debug)]
struct DMString {
    data: Vec<u8>,
}

#[derive(Debug)]
struct Misc {
    entries: Vec<u32>,
}

#[derive(Debug)]
struct Proc {
    path: Option<StringId>,
    name: Option<StringId>,
    desc: Option<StringId>,
    category: Option<StringId>,
    unk_1: u8,
    unk_2: u8,
    unk_3: u8,
    unk_4: Option<u32>,
    unk_5: Option<u8>,
    bytecode: MiscId,
    locals: MiscId,
    parameters: MiscId,
}

#[derive(Debug)]
struct Variable {
    kind: u8,
    value: u32,
    name: StringId,
}

#[derive(Debug)]
struct Instance {
    kind: u8,
    value: u32,
    initializer: Option<ProcId>,
}

#[derive(Debug)]
struct MapData {
    offset: u16,
    instance: Option<InstanceId>,
}

#[derive(Debug)]
struct World {
    mob: Option<MobId>,
    turf: Option<PathId>,
    area: Option<PathId>,
    procs: Option<MiscId>,
    initializer: Option<ProcId>,
    unk_0: Option<ObjectId>,
    name: StringId,
    unk_1: Option<ObjectId>,
    tick_lag_ms: u32,
    client: PathId,
    image: Option<PathId>,
    unk_2: u8,
    unk_3: u8,
    unk_4: Option<u16>,
    unk_5: u8,
    client_script: Option<ObjectId>,
    unk_7: Vec<ObjectId>,
    unk_8: Option<ObjectId>,
    unk_9: Option<u16>,
    unk_a: Option<u16>,
    unk_b: Option<u16>,
    hub_password: Option<StringId>,
    server_name: Option<StringId>,
    unk_c: Option<u32>,
    unk_d: Option<u32>,
    unk_e: Option<u16>,
    unk_f: Option<ObjectId>,
    unk_g: Option<ObjectId>,
    hub: Option<StringId>,
    channel: Option<StringId>,
    unk_h: Option<ObjectId>,
    icon_size_x: Option<u16>,
    icon_size_y: Option<u16>,
    unk_i: Option<u16>,
}

#[derive(Debug)]
struct File {
    id: u32,
    kind: u8,
}

#[derive(Copy, Clone, Debug)]
struct Parser<'a> {
    data: &'a [u8],
    header: Header,
}

impl<'a> Parser<'a> {
    fn new(i: &'a [u8]) -> Dmb {
        let data = i;

        // state-less parsing of header
        let (i, header) = header(i).unwrap();

        println!("header = {:?}", header);

        // force this on? goonstation needs this atm
        let mut header = header;
        //header.flags.large_object_ids = true;

        let parser = Self { data, header };

        let (i, grid) = parser.grid(i).unwrap();
        let (i, expected_string_bytes) = parser.string_bytes(i).unwrap();
        let (i, path_table) = parser.path_table(i).unwrap();
        let (i, mob_table) = parser.mob_table(i).unwrap();
        let (i, (string_table, actual_string_bytes)) = parser.string_table(i).unwrap();
        let (i, misc_table) = parser.misc_table(i).unwrap();
        let (i, proc_table) = parser.proc_table(i).unwrap();
        let (i, variable_table) = parser.variable_table(i).unwrap();
        let (i, some_proc_table) = parser.some_proc_table(i).unwrap();
        let (i, instance_table) = parser.instance_table(i).unwrap();
        let (i, map_data_table) = parser.map_data_table(i).unwrap();
        let (i, world) = parser.world(i).unwrap();
        let (i, file_table) = parser.file_table(i).unwrap();

        assert!(expected_string_bytes as usize == actual_string_bytes);
        assert!(i.is_empty());

        // TODO: validate string_bytes

        Dmb {
            grid,
            path_table,
            mob_table,
            string_table,
            misc_table,
            proc_table,
            variable_table,
            some_proc_table,
            instance_table,
            map_data_table,
            world,
            file_table,
        }
    }

    // i like to live dangerously
    fn offset(&self, i: &[u8]) -> usize {
        unsafe {
            i.as_ptr()
                .offset_from(self.data.as_ptr())
                .try_into()
                .unwrap()
        }
    }

    fn string_bytes(&self, i: &'a [u8]) -> IResult<&'a [u8], u32> {
        le_u32(i)
    }

    fn word(&self, i: &'a [u8]) -> IResult<&'a [u8], u32> {
        if self.header.flags.large_object_ids {
            return le_u32(i);
        }

        let (i, id) = le_u16(i)?;
        Ok((i, id as u32))
    }

    fn object<T: From<ObjectId>>(&self, i: &'a [u8]) -> IResult<&'a [u8], T> {
        let (i, id) = self.word(i)?;
        Ok((i, ObjectId(id).into()))
    }

    fn optional_object<T: From<ObjectId>>(&self, i: &'a [u8]) -> IResult<&'a [u8], Option<T>> {
        let (i, id) = self.word(i)?;

        if id == 0xFFFF {
            return Ok((i, None));
        }

        Ok((i, Some(ObjectId(id).into())))
    }

    fn grid(&self, i: &'a [u8]) -> IResult<&'a [u8], Grid> {
        let (i, z_width) = le_u16(i)?;
        let (i, z_height) = le_u16(i)?;
        let (i, z_count) = le_u16(i)?;

        let mut count = z_width as u64 * z_height as u64 * z_count as u64;

        let mut i = i;
        while count != 0 {
            let inner_i = i;
            let (inner_i, _turf) = self.word(inner_i)?;
            let (inner_i, _area) = self.word(inner_i)?;
            let (inner_i, _additional_turfs) = self.word(inner_i)?;
            let (inner_i, copies) = le_u8(inner_i)?;

            // goonstation hits this assert lol
            assert!(copies > 0);
            count = count.saturating_sub(copies as u64);
            i = inner_i;
        }

        Ok((i, Grid))
    }

    fn path_table(&self, i: &'a [u8]) -> IResult<&'a [u8], Vec<Path>> {
        let (i, count) = self.word(i)?;

        println!("Loading {} paths", count);

        let mut paths = vec![];
        paths.reserve(count.try_into().unwrap());

        let mut i = i;
        for _ in 0..count {
            let (inner_i, path) = self.path(i)?;
            paths.push(path);
            i = inner_i;
        }

        Ok((i, paths))
    }

    fn path(&self, i: &'a [u8]) -> IResult<&'a [u8], Path> {
        let (i, path) = self.object(i)?;
        let (i, parent) = self.optional_object(i)?;
        let (i, name) = self.optional_object(i)?;
        let (i, desc) = self.optional_object(i)?;
        let (i, icon) = self.optional_object(i)?;
        let (i, icon_state) = self.optional_object(i)?;
        let (i, direction) = le_u8(i)?;

        let (i, interface) = {
            let mut i = i;

            if self.header.major >= 307 {
                let res = le_u8(i)?;
                i = res.0;
                let mut interface = res.1 as u32;

                if interface == 0x0F {
                    let res = le_u32(i)?;
                    i = res.0;
                    interface = res.1;
                }

                (i, interface)
            } else {
                (i, 1)
            }
        };

        let (i, text) = self.optional_object(i)?;

        let (i, maptext, maptext_width, maptext_height) = if self.header.rhs >= 494 {
            let (i, maptext) = self.optional_object(i)?;
            let (i, maptext_width) = le_u16(i)?;
            let (i, maptext_height) = le_u16(i)?;
            (i, maptext, maptext_width, maptext_height)
        } else {
            (i, None, 0, 0) // TODO: HAS TO DEFAULT TO WORLD.ICON_SIZE?!?!?!
        };

        let (i, maptext_x, maptext_y) = if self.header.rhs >= 508 {
            let (i, maptext_x) = le_u16(i)?;
            let (i, maptext_y) = le_u16(i)?;
            (i, maptext_x, maptext_y)
        } else {
            (i, 0, 0)
        };

        let (i, suffix) = self.optional_object(i)?;

        let (i, flags) = if self.header.major >= 306 {
            if self.header.rhs >= 514 {
                // TODO: eck
                let (i, lo) = le_u32(i)?;
                let (i, hi) = le_u32(i)?;

                let mut lo = lo as u64;
                let mut hi = hi as u64;


                (i, lo | (hi << 32))
            } else {
                let (i, flags) = le_u32(i)?;
                (i, flags as u64)
            }
        } else {
            let (i, value) = le_u8(i)?;

            if self.header.rhs >= 0x203 {
                // TODO: check me
                unimplemented!()
            }

            (i, value as u64)
        };

        let (i, verbs) = self.optional_object(i)?;
        let (i, procs) = self.optional_object(i)?;
        let (i, initializer) = self.optional_object(i)?;
        let (i, initialized_vars) = self.optional_object(i)?;
        let (i, defining_vars) = self.optional_object(i)?;

        let (i, _something_else) = if self.header.rhs >= 514 {
            self.optional_object::<ObjectId>(i)?
        } else {
            (i, None)
        };

        let (i, layer) = if self.header.major >= 267 {
            le_f32(i)?
        } else {
            (i, 0.0)
        };

        let (i, transform) = if self.header.rhs >= 500 {
            let (i, has) = le_u8(i)?;

            if has != 0 {
                let mut transform: [f32; 6] = [0.0; 6];

                let mut i = i;
                for idx in 0..6 {
                    let res = le_f32(i)?;
                    i = res.0;
                    transform[idx] = res.1;
                }
                (i, Some(transform))
            } else {
                (i, None)
            }
        } else {
            (i, None)
        };

        let (i, color_matrix) = if self.header.rhs >= 509 {
            let (i, has) = le_u8(i)?;

            if has != 0 {
                let mut color_matrix: [f32; 20] = [0.0; 20];

                let mut i = i;
                for idx in 0..20 {
                    let res = le_f32(i)?;
                    i = res.0;
                    color_matrix[idx] = res.1;
                }
                (i, Some(color_matrix))
            } else {
                (i, None)
            }
        } else {
            (i, None)
        };

        let (i, overriding_vars) = if self.header.major >= 306 {
            let (i, overriding_vars) = self.optional_object(i)?;
            (i, overriding_vars)
        } else {
            (i, None)
        };

        Ok((
            i,
            Path {
                path,
                parent,
                name,
                desc,
                icon,
                icon_state,
                direction,
                interface,
                text,
                maptext,
                maptext_width,
                maptext_height,
                maptext_x,
                maptext_y,
                suffix,
                flags,
                verbs,
                procs,
                initializer,
                initialized_vars,
                defining_vars,
                layer,
                transform,
                color_matrix,
                overriding_vars,
            },
        ))
    }

    fn mob_table(&self, i: &'a [u8]) -> IResult<&'a [u8], Vec<Mob>> {
        let (i, count) = self.word(i)?;

        let mut mobs = vec![];
        mobs.reserve(count.try_into().unwrap());

        let mut i = i;
        for _ in 0..count {
            let (inner_i, mob) = self.mob(i)?;
            mobs.push(mob);
            i = inner_i;
        }

        Ok((i, mobs))
    }

    fn mob(&self, i: &'a [u8]) -> IResult<&'a [u8], Mob> {
        let (i, path) = self.object(i)?;
        let (i, key) = self.optional_object(i)?;
        let (i, sight_flags) = le_u8(i)?;

        let (i, sight_flags_ex, see_in_dark, see_invisible) = if sight_flags >= 0x80 {
            let (i, sight_flags_ex) = le_u32(i)?;
            let (i, see_in_dark) = le_u8(i)?;
            let (i, see_invisible) = le_u8(i)?;
            (
                i,
                Some(sight_flags_ex),
                Some(see_in_dark),
                Some(see_invisible),
            )
        } else {
            (i, None, None, None)
        };

        Ok((
            i,
            Mob {
                path,
                key,
                sight_flags,
                sight_flags_ex,
                see_in_dark,
                see_invisible,
            },
        ))
    }

    fn string_table(&self, i: &'a [u8]) -> IResult<&'a [u8], (Vec<DMString>, usize)> {
        let mut hash_state: i32 = -1;
        let mut total_bytes: usize = 0;

        let (i, count) = self.word(i)?;

        println!("loading {:?} strings", count);

        let mut strings = vec![];
        strings.reserve(count.try_into().unwrap());

        let mut i = i;
        for _ in 0..count {
            let (inner_i, string) = self.string(&mut hash_state, i)?;
            total_bytes += string.data.len() + 1;
            strings.push(string);
            i = inner_i;
        }

        let i = if self.header.major >= 468 {
            let (i, expected_hash) = le_i32(i)?;
            if expected_hash != hash_state {
                // goonstation hits this too HOW?!
                panic!("oh noooo");
            }
            i
        } else {
            i
        };

        Ok((i, (strings, total_bytes)))
    }

    fn string(&self, hash_state: &mut i32, i: &'a [u8]) -> IResult<&'a [u8], DMString> {
        let offset = self.offset(i);

        let (i, length) = le_u16(i)?;
        let length = length ^ ((offset & 0xFFFF) as u16);

        if length == 0xFFFF {
            unimplemented!();
        }

        let offset = self.offset(i);
        let (i, data) = take(length)(i)?;
        let data = xorjump::xorjump(offset as u8, data);

        nqcrc::hash(hash_state, &data);
        nqcrc::hash(hash_state, &[0]);

        Ok((i, DMString { data }))
    }

    fn misc_table(&self, i: &'a [u8]) -> IResult<&'a [u8], Vec<Misc>> {
        let (i, count) = self.word(i)?;

        println!("loading {:?} miscs", count);

        let mut miscs = vec![];
        miscs.reserve(count.try_into().unwrap());

        let mut i = i;
        for _ in 0..count {
            let (inner_i, misc) = self.misc(i)?;
            miscs.push(misc);
            i = inner_i;
        }

        Ok((i, miscs))
    }

    fn misc(&self, i: &'a [u8]) -> IResult<&'a [u8], Misc> {
        let (i, count) = le_u16(i)?;
        let mut entries = vec![];
        entries.reserve(count as usize);

        let mut i = i;
        for _ in 0..count {
            let res = self.word(i)?;
            i = res.0;
            entries.push(res.1);
        }

        Ok((i, Misc { entries }))
    }

    fn proc_table(&self, i: &'a [u8]) -> IResult<&'a [u8], Vec<Proc>> {
        let (i, count) = self.word(i)?;

        println!("loading {:?} procs", count);

        let mut procs = vec![];
        procs.reserve(count.try_into().unwrap());

        let mut i = i;
        for _ in 0..count {
            let (inner_i, proc) = self.proc(i)?;
            procs.push(proc);
            i = inner_i;
        }

        Ok((i, procs))
    }

    fn proc(&self, i: &'a [u8]) -> IResult<&'a [u8], Proc> {
        let (i, path) = if self.header.flags.large_object_ids || self.header.major >= 224 {
            self.optional_object(i)?
        } else {
            (i, None)
        };

        let (i, name) = self.optional_object(i)?;
        let (i, desc) = self.optional_object(i)?;
        let (i, category) = self.optional_object(i)?;
        let (i, unk_1) = le_u8(i)?;
        let (i, unk_2) = le_u8(i)?;
        let (i, unk_3) = le_u8(i)?;

        let (i, unk_4, unk_5) = if (unk_3 & 0x80) != 0 {
            let (i, unk_4) = le_u32(i)?;
            let (i, unk_5) = le_u8(i)?;
            (i, Some(unk_4), Some(unk_5))
        } else {
            (i, None, None)
        };

        let (i, bytecode) = self.object(i)?;
        let (i, locals) = self.object(i)?;
        let (i, parameters) = self.object(i)?;

        Ok((
            i,
            Proc {
                path,
                name,
                desc,
                category,
                unk_1,
                unk_2,
                unk_3,
                unk_4,
                unk_5,
                bytecode,
                locals,
                parameters,
            },
        ))
    }

    fn variable_table(&self, i: &'a [u8]) -> IResult<&'a [u8], Vec<Variable>> {
        let (i, count) = self.word(i)?;

        println!("loading {:?} variables", count);

        let mut variables = vec![];
        variables.reserve(count.try_into().unwrap());

        let mut i = i;
        for _ in 0..count {
            let (inner_i, proc) = self.variable(i)?;
            variables.push(proc);
            i = inner_i;
        }

        // global and world vars?
        let (i, unk_0) = if self.header.major >= 512 && self.header.lhs >= 512 {
            let (i, unk_0) = le_u32(i)?;
            (i, Some(unk_0))
        } else {
            (i, None)
        };

        println!("ignoring {:?}", unk_0);

        Ok((i, variables))
    }

    fn variable(&self, i: &'a [u8]) -> IResult<&'a [u8], Variable> {
        let (i, kind) = le_u8(i)?;
        let (i, value) = le_u32(i)?;
        let (i, name) = self.object(i)?;

        Ok((i, Variable { kind, value, name }))
    }

    fn some_proc_table(&self, i: &'a [u8]) -> IResult<&'a [u8], Vec<ProcId>> {
        let (i, count) = self.word(i)?;

        println!("loading {:?} some_procs", count);

        let mut variables = vec![];
        variables.reserve(count.try_into().unwrap());

        let mut i = i;
        for _ in 0..count {
            let (inner_i, proc) = self.object(i)?;
            variables.push(proc);
            i = inner_i;
        }

        Ok((i, variables))
    }

    fn instance_table(&self, i: &'a [u8]) -> IResult<&'a [u8], Vec<Instance>> {
        let (i, count) = self.word(i)?;

        println!("loading {:?} instances", count);

        let mut instances = vec![];
        instances.reserve(count.try_into().unwrap());

        let mut i = i;
        for _ in 0..count {
            let (inner_i, instance) = self.instance(i)?;
            instances.push(instance);
            i = inner_i;
        }

        Ok((i, instances))
    }

    fn instance(&self, i: &'a [u8]) -> IResult<&'a [u8], Instance> {
        let (i, kind) = le_u8(i)?;
        let (i, value) = le_u32(i)?;
        let (i, initializer) = self.optional_object(i)?;

        Ok((
            i,
            Instance {
                kind,
                value,
                initializer,
            },
        ))
    }

    fn map_data_table(&self, i: &'a [u8]) -> IResult<&'a [u8], Vec<MapData>> {
        let (i, count) = le_u32(i)?;

        println!("loading {:?} map datas", count);

        let mut map_datas = vec![];
        map_datas.reserve(count.try_into().unwrap());

        let mut i = i;
        for _ in 0..count {
            let (inner_i, map_data) = self.map_data(i)?;
            map_datas.push(map_data);
            i = inner_i;
        }

        Ok((i, map_datas))
    }

    fn map_data(&self, i: &'a [u8]) -> IResult<&'a [u8], MapData> {
        let (i, offset) = le_u16(i)?;
        let (i, instance) = self.optional_object(i)?;

        Ok((i, MapData { offset, instance }))
    }

    fn world(&self, i: &'a [u8]) -> IResult<&'a [u8], World> {
        let (i, mob) = self.optional_object(i)?;
        let (i, turf) = self.optional_object(i)?;
        let (i, area) = self.optional_object(i)?;
        let (i, procs) = self.optional_object(i)?;
        let (i, initializer) = self.optional_object(i)?;
        let (i, unk_0) = self.optional_object(i)?;
        let (i, name) = self.object(i)?;

        let (i, unk_1) = if self.header.major < 368 {
            let (i, unk_1) = self.object(i)?;
            (i, Some(unk_1))
        } else {
            (i, None)
        };

        let (i, tick_lag_ms) = le_u32(i)?;
        let (i, client) = self.object(i)?;

        let (i, image) = if self.header.major >= 308 {
            let (i, image) = self.object(i)?;
            (i, Some(image))
        } else {
            (i, None)
        };

        let (i, unk_2) = le_u8(i)?;
        let (i, unk_3) = le_u8(i)?;

        let (i, unk_4) = if self.header.major >= 415 {
            let (i, unk_4) = le_u16(i)?;
            (i, Some(unk_4))
        } else {
            (i, None)
        };

        let (i, unk_5) = le_u8(i)?;

        let (i, client_script) = if self.header.major >= 230 {
            let (i, client_script) = self.object(i)?;
            (i, Some(client_script))
        } else {
            (i, None)
        };

        let (i, unk_7) = if self.header.major >= 507 {
            let (i, count) = le_u16(i)?;
            let mut unk_7 = vec![];
            unk_7.reserve(count as usize);

            let mut i = i;
            for _ in 0..count {
                let res = self.object(i)?;
                i = res.0;
                unk_7.push(res.1);
            }

            (i, unk_7)
        } else {
            (i, vec![])
        };

        let (i, unk_8) = if self.header.major < 507 {
            self.optional_object(i)?
        } else {
            (i, None)
        };

        let (i, unk_9) = if self.header.major >= 232 {
            let (i, unk_9) = le_u16(i)?;
            (i, Some(unk_9))
        } else {
            (i, None)
        };

        let (i, unk_a) = if self.header.major >= 235 && self.header.major < 368 {
            let (i, unk_a) = le_u16(i)?;
            (i, Some(unk_a))
        } else {
            (i, None)
        };

        let (i, unk_b) = if self.header.major >= 236 && self.header.major < 368 {
            let (i, unk_b) = le_u16(i)?;
            (i, Some(unk_b))
        } else {
            (i, None)
        };

        let (i, hub_password) = if self.header.major >= 341 {
            self.optional_object(i)?
        } else {
            (i, None)
        };

        let (i, server_name, unk_c, unk_d) = if self.header.major >= 266 {
            let (i, server_name) = self.optional_object(i)?;
            let (i, unk_c) = le_u32(i)?;
            let (i, unk_d) = le_u32(i)?;
            (i, server_name, Some(unk_c), Some(unk_d))
        } else {
            (i, None, None, None)
        };

        let (i, unk_e, unk_f, unk_g) = if self.header.major >= 272 {
            let (i, unk_e) = le_u16(i)?;
            let (i, unk_f) = self.optional_object(i)?;
            let (i, unk_g) = self.optional_object(i)?;
            (i, Some(unk_e), unk_f, unk_g)
        } else {
            (i, None, None, None)
        };

        let (i, hub) = if self.header.major >= 276 {
            self.optional_object(i)?
        } else {
            (i, None)
        };

        let (i, channel) = if self.header.major >= 305 {
            self.optional_object(i)?
        } else {
            (i, None)
        };

        let (i, unk_h) = if self.header.major >= 360 {
            self.optional_object(i)?
        } else {
            (i, None)
        };

        let (i, icon_size_x, icon_size_y, unk_i) = if self.header.major >= 272 {
            let (i, icon_size_x) = le_u16(i)?;
            let (i, icon_size_y) = le_u16(i)?;
            let (i, unk_i) = le_u16(i)?;
            (i, Some(icon_size_x), Some(icon_size_y), Some(unk_i))
        } else {
            (i, None, None, None)
        };

        Ok((
            i,
            World {
                mob,
                turf,
                area,
                procs,
                initializer,
                unk_0,
                name,
                unk_1,
                tick_lag_ms,
                client,
                image,
                unk_2,
                unk_3,
                unk_4,
                unk_5,
                client_script,
                unk_7,
                unk_8,
                unk_9,
                unk_a,
                unk_b,
                hub_password,
                server_name,
                unk_c,
                unk_d,
                unk_e,
                unk_f,
                unk_g,
                hub,
                channel,
                unk_h,
                icon_size_x,
                icon_size_y,
                unk_i,
            },
        ))
    }

    fn file_table(&self, i: &'a [u8]) -> IResult<&'a [u8], Vec<File>> {
        let (i, count) = self.word(i)?;

        println!("loading {:?} files", count);

        let mut files = vec![];
        files.reserve(count.try_into().unwrap());

        let mut i = i;
        for _ in 0..count {
            let (inner_i, file) = self.file(i)?;
            files.push(file);
            i = inner_i;
        }

        Ok((i, files))
    }

    fn file(&self, i: &'a [u8]) -> IResult<&'a [u8], File> {
        let (i, id) = le_u32(i)?;
        let (i, kind) = le_u8(i)?;

        Ok((i, File { id, kind }))
    }
}

#[derive(Copy, Clone, Debug)]
struct Header {
    major: u32,
    lhs: u32,
    rhs: u32,
    flags: Flags,
}

#[derive(Copy, Clone, Debug)]
struct Flags {
    large_object_ids: bool,
}

/// parses until a non-numeric character is hit, we might be using this in places that we shouldn't
fn parse_plaintext_uint(i: &[u8]) -> IResult<&[u8], u32> {
    map_res(take_while(is_digit), |x: &[u8]| {
        let string = std::str::from_utf8(x).unwrap();
        string.parse::<u32>()
    })(i)
}

// NOTE: older versions of byond have a different format here
fn header(i: &[u8]) -> IResult<&[u8], Header> {
    // TODO: shebang mess
    map(
        tuple((
            delimited(tag("world bin v"), parse_plaintext_uint, tag("\x0A")),
            delimited(tag("min compatibility v"), parse_plaintext_uint, tag(" ")),
            terminated(parse_plaintext_uint, tag("\n")),
            header_flags,
        )),
        |(major, lhs, rhs, flags)| Header {
            major,
            lhs,
            rhs,
            flags,
        },
    )(i)
}

fn header_flags(i: &[u8]) -> IResult<&[u8], Flags> {
    alt((
        map_res(le_u32, |flags| {
            if (flags & (1 << 31)) != 0 {
                return Err(());
            }

            Ok(Flags {
                large_object_ids: (flags & (1 << 30) != 0),
            })
        }),
        map(preceded(le_u32, le_u32), |flags| Flags {
            large_object_ids: (flags & (1 << 30) != 0),
        }),
    ))(i)
}

#[cfg(test)]
mod tests {
    use super::*;

   const EXAMPLE_DMB: &'static [u8] = include_bytes!("E:\\spantest_char_crash\\spantest_char_crash.dmb");
   // const EXAMPLE_DMB: &'static [u8] = include_bytes!("E:\\tgstation\\tgstation.dmb");
   //  const EXAMPLE_DMB: &'static [u8] = include_bytes!("E:\\goonstation\\goonstation.dmb");

    #[test]
    fn it_works() {
        let dmb = super::Parser::new(EXAMPLE_DMB);

        fn debug(dmb: &super::Dmb, id: ObjectId) {
            println!("Debugging {:x?}", id);
            println!("\tString = {:?}", String::from_utf8_lossy(dmb.string(StringId(id))));
            println!("\tProc = {:?}", dmb.proc(ProcId(id)));
        }

        for path in &dmb.path_table {
            let res = super::PathFlags::from_bits(path.flags);

            match res {
                Some(flags) => {
                    if path.flags > 0 {
                       println!("{:#x?} = {:x?}", String::from_utf8_lossy(dmb.string(path.path)), flags);
                    }
                }

                None => {
                    let res2 = super::PathFlags::from_bits_truncate(path.flags);
                    println!("UNKNOWN: {:#x?} = {} ({:?}, {})", String::from_utf8_lossy(dmb.string(path.path)), path.flags, res2, path.flags ^ res2.bits);
                    // break;
                }
            }

           //println!("{:#x?}", path);

           //break;
        }

        //println!("{:#x?}", dmb.world);

        // debug(&dmb, dmb.world.client_script.unwrap());
    }
}
