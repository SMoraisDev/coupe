use anyhow::Context as _;
use anyhow::Result;
use mesh_io::ElementType;
use std::env;
use std::fs;
use std::io;

const USAGE: &str = "Usage: apply-part [options] [out-mesh] >out.mesh";

fn main() -> Result<()> {
    let mut options = getopts::Options::new();
    options.optflag("h", "help", "print this help menu");
    options.optflag("", "version", "print version information");
    options.optopt("f", "format", "output format", "EXT");
    options.optopt("m", "mesh", "mesh file", "FILE");
    options.optopt("p", "partition", "partition file", "FILE");

    let matches = options.parse(env::args().skip(1))?;

    if matches.opt_present("h") {
        println!("{}", options.usage(USAGE));
        return Ok(());
    }
    if matches.opt_present("version") {
        println!("apply-part version {}", env!("COUPE_VERSION"));
        return Ok(());
    }
    if matches.free.len() > 1 {
        anyhow::bail!("too many arguments\n\n{}", options.usage(USAGE));
    }

    let format = matches
        .opt_get("f")
        .context("invalid value for option 'format'")?;

    let mesh_file = matches
        .opt_str("m")
        .context("missing required option 'mesh'")?;
    let mut mesh = mesh_io::Mesh::from_file(mesh_file).context("failed to read mesh file")?;

    let partition_file = matches
        .opt_str("p")
        .context("missing required option 'partition'")?;
    let partition_file = fs::File::open(partition_file).context("failed to open partition file")?;
    let partition_file = io::BufReader::new(partition_file);
    let parts =
        mesh_io::partition::read(partition_file).context("failed to read partition file")?;

    if let Some(element_dim) = mesh
        .topology()
        .iter()
        .map(|(el_type, _, _)| el_type.dimension())
        .max()
    {
        mesh.elements_mut()
            .filter(|(element_type, _, _)| {
                element_type.dimension() == element_dim && *element_type != ElementType::Edge
            })
            .zip(parts)
            .for_each(|((_, _, element_ref), part)| *element_ref = part as isize);
    }

    coupe_tools::write_mesh(&mesh, format, matches.free.get(0))?;

    Ok(())
}
