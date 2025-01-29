import 'package:chalkydri_manager/main.dart';
import 'package:chalkydri_manager/menu.dart';
import 'package:flutter/material.dart';

class ChalkydriDevices extends StatefulWidget {
	const ChalkydriDevices({super.key});

	@override
	State<ChalkydriDevices> createState() => _ChalkydriDevicesState();
}
class _ChalkydriDevicesState extends State<ChalkydriDevices> {
	@override
	Widget build(BuildContext context) {
		return Scaffold(
			appBar: buildAppBar(context),
			drawer: ChalkydriMenu(),
			body: Center(),
			floatingActionButton: FloatingActionButton(
				child: const Icon(Icons.add),
				onPressed: () {
					Navigator.pushNamed(context, '/devices/add');
				},
			),
		);
	}
}

class ChalkydriAddDevice extends StatefulWidget {
	const ChalkydriAddDevice({super.key});

	@override
	State<ChalkydriAddDevice> createState() => _ChalkydriAddDeviceState();
}
class _ChalkydriAddDeviceState extends State<ChalkydriAddDevice> {
	@override
	Widget build(BuildContext context) {
		return Scaffold(
			appBar: buildAppBar(context),
			drawer: ChalkydriMenu(),
			body: Center(),
		);
	}
}
