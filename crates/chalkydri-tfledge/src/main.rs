use std::{fs::File, io::Read, time::Instant};

use tfledge::{list_devices, Error, Interpreter, Model};

fn main() -> Result<(), Error> {
    env_logger::init();
    let m = Model::from_file("Note_Detector.tflite")?;

    let d = list_devices().next().unwrap();
    println!("{}", d.path());
    let mut int = Interpreter::new(m, d).unwrap();

    let mut input = int.input_tensor::<f32>(0);
    //let output = int.output_tensor(0);

    println!("ity: {:?}", input.kind());
    //println!("oty: {:?}", output.kind());

    assert_eq!(input.num_dims().unwrap(), 4);

    let mut buf: Vec<u8> = Vec::new();
    File::open("test.rgb")
        .unwrap()
        .read_to_end(&mut buf)
        .unwrap();
    input.write(&buf).unwrap();

    println!("{}", input.num_dims().unwrap());
    for dim in 0..input.num_dims().unwrap() {
        println!("- {}", input.dim(dim));
    }

    for _ in 0..100_000 {
        let st = Instant::now();
        int.invoke()?;

        let boxes = int.output_tensor::<f32>(1).read::<4>();
        let classes = int.output_tensor::<f32>(3).read::<1>();
        let scores = int.output_tensor::<f32>(0).read::<1>();

        for aaa in boxes {
            println!("{aaa:?}");
        }

        for aaa in classes {
            println!("{aaa:?}");
        }

        for aaa in scores {
            println!("{aaa:?}");
        }

        /*
        for (label, output, chunksz) in [
            ("boxes", int.output_tensor(1), 4),
            ("classes", int.output_tensor(3), 1),
            ("scores", int.output_tensor(0), 1),
        ] {
            println!("[{label}] ({:?})", output.kind());
            println!("{}", output.num_dims());
            for dim in 0..output.num_dims() {
                println!("- {}", output.dim(dim));
            }

            for aaa in output.read::<chunksz>() {
                println!("{aaa:?}");
            }
        }
        */
        println!("{:?}", st.elapsed());
    }

    Ok(())
}
