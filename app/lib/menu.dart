import 'package:chalkydri_manager/utils.dart';
import 'package:flutter/material.dart';

enum ChalkydriMenuPage {
  home,
  stats,
  config_device,
  config_camera,
  config_apriltags,
  config_machine_learning,
  config_python,
}

class ChalkydriMenu extends StatefulWidget {
  const ChalkydriMenu({super.key});

  @override
  State<ChalkydriMenu> createState() => _ChalkydriMenuState();
}

class _ChalkydriMenuState extends State<ChalkydriMenu> {
  static var page = ChalkydriMenuPage.home;

  @override
  Widget build(BuildContext context) {
    //return MenuBar(
    //  children: [
    //    MenuItemButton(
    //      child: Text('Device'),
    //      onPressed: () {
    //        page = ChalkydriMenuPage.home;
    //      },
    //    ),
    //  ],
    //);
		return Drawer(
			child: ListView(
				children: [
					ListTile(
						title: const Text('Home'),
						onTap: () {
							Navigator.pushNamed(context, '/');
						},
					),
					if (isDS()) ListTile(
						title: const Text('Devices'),
						onTap: () {
							Navigator.pushNamed(context, '/devices');
						},
					),
				],
			),
		);
  }
}
