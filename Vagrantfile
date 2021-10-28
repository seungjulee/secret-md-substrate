# -*- mode: ruby -*-
# vi: set ft=ruby :

VAGRANTFILE_API_VERSION = "2"

Vagrant.configure(VAGRANTFILE_API_VERSION) do |config|
  config.vm.provider "hyperv" do |hv|
    hv.cpus = "2"
    hv.memory = "8192"
    hv.enable_enhanced_session_mode = true
  end

  # https://www.vagrantup.com/docs/providers/virtualbox
  config.vm.provider "virtualbox" do |vb|
    vb.cpus = "4"
    vb.memory = "8192"
  end
    config.vm.box = "generic/ubuntu2004"
    config.vm.box_version = "3.1.16"
    config.vm.provision:shell, inline: <<-SHELL
        echo "root:rootroot" | sudo chpasswd
        # sudo timedatectl set-timezone Asia/Ho_Chi_Minh
    SHELL


    # config.vm.define "ubuntu20.04" do |ubuntu|
    #     ubuntu.vm.hostname = "ubuntu20.04"
    # end

    # cfg.vm.synced_folder ".", "/vagrant", disabled: true

    config.vm.network "private_network", ip: "192.168.33.10"

    config.vm.network "forwarded_port", guest: 8000, host: 8000
    # config.vm.network "forwarded_port", guest: 8080, host: 8080
    # config.vm.network "forwarded_port", guest: 9933, host: 9933
    # config.vm.network "forwarded_port", guest: 9944, host: 9944
    # config.vm.network "forwarded_port", guest: 9944, host: 9944, id: "dot", host_ip: "0.0.0.0"
    config.vm.network "forwarded_port", guest: 9944, host: 9944, id: "dot", host_ip: "192.168.33.10"

    # config.vm.network "forwarded_port", guest: 9944, host: 9955
    config.vm.network "forwarded_port", guest: 30333, host: 30333

    # frontend
    config.vm.network "forwarded_port", guest: 8001, host: 8001
    # config.vm.network "forwarded_port", guest: 19944, host: 19944
    config.vm.network "forwarded_port", guest: 3000, host: 3000


    config.vm.synced_folder ".", "/vagrant", type: "virtualbox"
    config.vm.synced_folder "../js-sdk", "/js-sdk", type: "virtualbox"


    config.vm.provider :virtualbox do |vb|
        vb.gui = true
      end

    # /Users/seungjulee/code/hackathon/encode.club/polkadot21/phala/js-sdk

    # config.vm.provision:shell, path: "bootstrap.sh"
end
